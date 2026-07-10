//! Android JNI boundary for the coarse `JniBindings` search-session API.
//!
//! JNI owns handles, validation, polling packets, and lifecycle. World workers
//! never call back into the JVM.

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JLongArray};
use jni::sys::{jint, jlong};
use shpd_seedfinder_core::main_world::CanonicalMainWorldGenerator;
use shpd_seedfinder_core::query::SearchQuery;
use shpd_seedfinder_core::search::{
    SearchError, SearchOptions, StreamingSearchHandle, StreamingSearchState, WorldGenerator,
    spawn_streaming_search,
};
use shpd_seedfinder_core::seed::{DungeonSeed, TOTAL_SEEDS};
use shpd_seedfinder_core::wire::{
    WireError, decode_query, decode_scout_seed, encode_results, encode_scout_world,
};

const STATE_RUNNING: jlong = 0;
const STATE_COMPLETED: jlong = 1;
const STATE_CANCELLED: jlong = 2;
const STATE_FAILED: jlong = 3;

const ERROR_NONE: jlong = 0;
/// A canonical generation worker panicked. Kept stable for Android diagnostics.
const ERROR_SEARCH_WORKER_FAILED: jlong = 2_001;

/// Four is the native batch/SIMD width and bounds cooperative-cancel latency to
/// one depth-24 batch. Generation dominates the atomic claim overhead.
const SEARCH_CHUNK_SIZE: usize = 4;
/// Prevent an unattended search from retaining an unbounded result queue.
const MAX_ACCEPTED_RESULTS: usize = 1_024;

static REGISTRY: OnceLock<SessionRegistry> = OnceLock::new();
static CANONICAL_GENERATOR: OnceLock<Arc<CanonicalMainWorldGenerator>> = OnceLock::new();

fn registry() -> &'static SessionRegistry {
    REGISTRY.get_or_init(SessionRegistry::new)
}

fn canonical_generator() -> &'static Arc<CanonicalMainWorldGenerator> {
    CANONICAL_GENERATOR.get_or_init(|| Arc::new(CanonicalMainWorldGenerator))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScoutPacketError {
    Request(WireError),
    Response(WireError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScoutCallError {
    Packet(ScoutPacketError),
    Panicked,
}

/// Validates one user seed, evaluates exactly one canonical depth-24 world,
/// and encodes its complete searchable item list for the Android scout UI.
fn scout_seed_packet<G: WorldGenerator + ?Sized>(
    generator: &G,
    request: &[u8],
) -> Result<Vec<u8>, ScoutPacketError> {
    let seed = decode_scout_seed(request).map_err(ScoutPacketError::Request)?;
    let world = generator.generate(seed, 24);
    encode_scout_world(&world).map_err(ScoutPacketError::Response)
}

fn protected_scout_seed_packet<G: WorldGenerator + ?Sized>(
    generator: &G,
    request: &[u8],
) -> Result<Vec<u8>, ScoutCallError> {
    catch_unwind(AssertUnwindSafe(|| scout_seed_packet(generator, request)))
        .map_err(|_| ScoutCallError::Panicked)?
        .map_err(ScoutCallError::Packet)
}

struct NativeSession {
    search: StreamingSearchHandle,
    failure_logged: AtomicBool,
}

impl NativeSession {
    fn start<G: WorldGenerator + Send + 'static>(
        generator: &Arc<G>,
        query: SearchQuery,
        options: SearchOptions,
    ) -> Result<Self, SearchError> {
        spawn_streaming_search(generator, query, options).map(|search| Self {
            search,
            failure_logged: AtomicBool::new(false),
        })
    }

    fn production(query: SearchQuery) -> Result<Self, SearchError> {
        let options = SearchOptions {
            start_seed: 0,
            end_seed_exclusive: TOTAL_SEEDS,
            workers: SearchOptions::available_parallelism(),
            chunk_size: NonZeroUsize::new(SEARCH_CHUNK_SIZE)
                .expect("the search chunk size is non-zero"),
            max_results: NonZeroUsize::new(MAX_ACCEPTED_RESULTS)
                .expect("the result limit is non-zero"),
        };
        Self::start(canonical_generator(), query, options)
    }

    fn poll(&self, maximum: usize) -> Result<Vec<u8>, WireError> {
        encode_results(&self.search.drain_results(maximum))
    }

    fn cancel(&self) {
        self.search.cancel();
    }

    fn status(&self) -> [jlong; 4] {
        let (state, error) = match self.search.state() {
            StreamingSearchState::Running => (STATE_RUNNING, ERROR_NONE),
            StreamingSearchState::Completed => (STATE_COMPLETED, ERROR_NONE),
            StreamingSearchState::Cancelled => (STATE_CANCELLED, ERROR_NONE),
            StreamingSearchState::Failed => {
                if !self.failure_logged.swap(true, Ordering::AcqRel) {
                    if let Some(diagnostic) = worker_failure_diagnostic(&self.search) {
                        android_error(&diagnostic);
                    }
                }
                (STATE_FAILED, ERROR_SEARCH_WORKER_FAILED)
            }
        };
        [
            state,
            jlong::try_from(self.search.tested()).unwrap_or(jlong::MAX),
            jlong::try_from(self.search.total()).unwrap_or(jlong::MAX),
            error,
        ]
    }
}

fn worker_failure_diagnostic(search: &StreamingSearchHandle) -> Option<String> {
    let failure = search.failure()?;
    let range = match (failure.chunk_start, failure.chunk_end_exclusive) {
        (Some(start), Some(end)) => {
            let first = DungeonSeed::new(start)
                .map_or_else(|_| start.to_string(), |seed| format!("{start} ({seed})"));
            let last = end.checked_sub(1).map_or_else(
                || "unknown".to_owned(),
                |value| {
                    DungeonSeed::new(value)
                        .map_or_else(|_| value.to_string(), |seed| format!("{value} ({seed})"))
                },
            );
            format!("{first}..={last}")
        }
        _ => "unknown".to_owned(),
    };
    let message = failure.message.replace('\0', "\\0").replace('\n', "\\n");
    Some(format!(
        "streaming worker panic in seed chunk {range}: {message}"
    ))
}

#[cfg(target_os = "android")]
fn android_error(message: &str) {
    use std::ffi::{CString, c_char, c_int};

    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    const ANDROID_LOG_ERROR: c_int = 6;
    let Ok(tag) = CString::new("SeedFinderNative") else {
        return;
    };
    let Ok(text) = CString::new(message) else {
        return;
    };
    // SAFETY: both pointers remain valid NUL-terminated C strings for the
    // duration of Android's synchronous logging call.
    unsafe {
        __android_log_write(ANDROID_LOG_ERROR, tag.as_ptr(), text.as_ptr());
    }
}

#[cfg(not(target_os = "android"))]
fn android_error(_message: &str) {}

struct SessionRegistry {
    next_handle: AtomicI64,
    sessions: Mutex<HashMap<jlong, Arc<NativeSession>>>,
}

impl SessionRegistry {
    fn new() -> Self {
        Self {
            next_handle: AtomicI64::new(1),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn insert(&self, session: NativeSession) -> jlong {
        let session = Arc::new(session);
        loop {
            let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
            if handle == 0 {
                continue;
            }
            let mut guard = self
                .sessions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let std::collections::hash_map::Entry::Vacant(entry) = guard.entry(handle) {
                entry.insert(session);
                return handle;
            }
        }
    }

    fn get(&self, handle: jlong) -> Option<Arc<NativeSession>> {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&handle)
            .cloned()
    }

    fn remove(&self, handle: jlong) -> Option<Arc<NativeSession>> {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&handle)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

/// Removes visibility under the short registry lock, then cancels and joins
/// workers when the returned `Arc` is dropped outside that lock.
fn close_session(registry: &SessionRegistry, handle: jlong) -> bool {
    let removed = registry.remove(handle);
    if let Some(session) = removed {
        drop(session);
        true
    } else {
        false
    }
}

fn throw_illegal_argument(env: &mut JNIEnv<'_>, message: impl AsRef<str>) {
    let _ = env.throw_new("java/lang/IllegalArgumentException", message.as_ref());
}

fn throw_illegal_state(env: &mut JNIEnv<'_>, message: impl AsRef<str>) {
    let _ = env.throw_new("java/lang/IllegalStateException", message.as_ref());
}

/// `JniBindings.scoutSeed(byte[])`.
///
/// Generation is synchronous at this boundary. The Kotlin adapter invokes it
/// from `Dispatchers.Default`, keeping JNI free of JVM callbacks and handles.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_seedseeker_app_engine_JniBindings_scoutSeed<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    request: JByteArray<'local>,
) -> JByteArray<'local> {
    let bytes = match env.convert_byte_array(&request) {
        Ok(bytes) => bytes,
        Err(error) => {
            throw_illegal_argument(&mut env, format!("invalid seed request array: {error}"));
            return JByteArray::default();
        }
    };
    let packet = match protected_scout_seed_packet(canonical_generator().as_ref(), &bytes) {
        Ok(packet) => packet,
        Err(ScoutCallError::Packet(ScoutPacketError::Request(error))) => {
            throw_illegal_argument(&mut env, error.to_string());
            return JByteArray::default();
        }
        Err(ScoutCallError::Packet(ScoutPacketError::Response(error))) => {
            throw_illegal_state(&mut env, format!("cannot encode scout response: {error}"));
            return JByteArray::default();
        }
        Err(ScoutCallError::Panicked) => {
            android_error("canonical depth-24 scouting generation panicked");
            throw_illegal_state(&mut env, "native scouting generation failed");
            return JByteArray::default();
        }
    };
    match env.byte_array_from_slice(&packet) {
        Ok(array) => array,
        Err(error) => {
            throw_illegal_state(&mut env, format!("cannot allocate scout response: {error}"));
            JByteArray::default()
        }
    }
}

/// `JniBindings.startSearch(byte[])`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_seedseeker_app_engine_JniBindings_startSearch<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    request: JByteArray<'local>,
) -> jlong {
    let bytes = match env.convert_byte_array(&request) {
        Ok(bytes) => bytes,
        Err(error) => {
            throw_illegal_argument(&mut env, format!("invalid request array: {error}"));
            return 0;
        }
    };
    let query = match decode_query(&bytes) {
        Ok(query) => query,
        Err(error) => {
            throw_illegal_argument(&mut env, error.to_string());
            return 0;
        }
    };
    let session = match NativeSession::production(query) {
        Ok(session) => session,
        Err(error) => {
            throw_illegal_state(&mut env, format!("cannot start native search: {error:?}"));
            return 0;
        }
    };

    registry().insert(session)
}

/// `JniBindings.poll(long, int)`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_seedseeker_app_engine_JniBindings_poll<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    max_results: jint,
) -> JByteArray<'local> {
    if !(1..=1024).contains(&max_results) {
        throw_illegal_argument(&mut env, "maxResults must be 1..=1024");
        return JByteArray::default();
    }
    let Some(session) = registry().get(handle) else {
        throw_illegal_state(&mut env, "unknown or closed native search handle");
        return JByteArray::default();
    };
    let packet = match session.poll(usize::try_from(max_results).unwrap_or_default()) {
        Ok(packet) => packet,
        Err(error) => {
            throw_illegal_state(&mut env, format!("cannot encode result packet: {error}"));
            return JByteArray::default();
        }
    };
    match env.byte_array_from_slice(&packet) {
        Ok(array) => array,
        Err(error) => {
            throw_illegal_state(&mut env, format!("cannot allocate result packet: {error}"));
            JByteArray::default()
        }
    }
}

/// `JniBindings.status(long)`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_seedseeker_app_engine_JniBindings_status<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> JLongArray<'local> {
    let Some(session) = registry().get(handle) else {
        throw_illegal_state(&mut env, "unknown or closed native search handle");
        return JLongArray::default();
    };
    let status = session.status();
    let array = match env.new_long_array(4) {
        Ok(array) => array,
        Err(error) => {
            throw_illegal_state(&mut env, format!("cannot allocate status array: {error}"));
            return JLongArray::default();
        }
    };
    if let Err(error) = env.set_long_array_region(&array, 0, &status) {
        throw_illegal_state(&mut env, format!("cannot populate status array: {error}"));
        return JLongArray::default();
    }
    array
}

/// `JniBindings.cancel(long)`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_seedseeker_app_engine_JniBindings_cancel<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
) {
    let Some(session) = registry().get(handle) else {
        throw_illegal_state(&mut env, "unknown or closed native search handle");
        return;
    };
    session.cancel();
}

/// `JniBindings.close(long)`. Repeated close is deliberately safe.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_seedseeker_app_engine_JniBindings_close<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
) {
    close_session(registry(), handle);
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};

    use shpd_seedfinder_core::catalog::{ItemId, ItemKind};
    use shpd_seedfinder_core::model::{Accessibility, GeneratedWorld, ItemSource, WorldItem};
    use shpd_seedfinder_core::query::{Requirement, SearchQuery};
    use shpd_seedfinder_core::search::{SearchOptions, WorldGenerator};
    use shpd_seedfinder_core::seed::DungeonSeed;
    use shpd_seedfinder_core::wire::{WireError, decode_scout_world};

    use super::{
        ERROR_NONE, ERROR_SEARCH_WORKER_FAILED, NativeSession, STATE_CANCELLED, STATE_COMPLETED,
        STATE_FAILED, STATE_RUNNING, ScoutCallError, ScoutPacketError, SessionRegistry,
        close_session, protected_scout_seed_packet, scout_seed_packet, worker_failure_diagnostic,
    };

    struct MatchingGenerator;

    impl WorldGenerator for MatchingGenerator {
        fn generate(&self, seed: DungeonSeed, _max_depth: u8) -> GeneratedWorld {
            matching_world(seed)
        }
    }

    struct PanickingGenerator;

    impl WorldGenerator for PanickingGenerator {
        fn generate(&self, _seed: DungeonSeed, _max_depth: u8) -> GeneratedWorld {
            panic!("intentional worker failure")
        }
    }

    #[derive(Default)]
    struct RecordingScoutGenerator {
        calls: AtomicUsize,
        inputs: Mutex<Vec<(DungeonSeed, u8)>>,
    }

    impl WorldGenerator for RecordingScoutGenerator {
        fn generate(&self, seed: DungeonSeed, max_depth: u8) -> GeneratedWorld {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.inputs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((seed, max_depth));
            matching_world(seed)
        }
    }

    #[derive(Default)]
    struct Gate {
        open: Mutex<bool>,
        changed: Condvar,
    }

    impl Gate {
        fn open(&self) {
            *self
                .open
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            self.changed.notify_all();
        }

        fn wait(&self) {
            let guard = self
                .open
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(
                self.changed
                    .wait_while(guard, |open| !*open)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }

        fn wait_until_open(&self) {
            self.wait();
        }
    }

    struct GatedGenerator {
        entered: Arc<Gate>,
        release: Arc<Gate>,
    }

    impl WorldGenerator for GatedGenerator {
        fn generate(&self, seed: DungeonSeed, _max_depth: u8) -> GeneratedWorld {
            self.entered.open();
            self.release.wait();
            matching_world(seed)
        }
    }

    fn matching_world(seed: DungeonSeed) -> GeneratedWorld {
        GeneratedWorld {
            seed,
            items: vec![WorldItem {
                item: ItemId::WandFrost,
                upgrade: 2,
                effect: None,
                cursed: false,
                depth: 1,
                source: ItemSource::Heap,
                accessibility: Accessibility::Independent,
            }],
        }
    }

    fn query() -> SearchQuery {
        SearchQuery {
            requirements: vec![Requirement {
                kind: ItemKind::Wand,
                item: Some(ItemId::WandFrost),
                upgrade: shpd_seedfinder_core::query::UpgradeRequirement::Exact(2),
                effect: None,
                source: None,
                identity_group: None,
            }],
            max_depth: 24,
            require_blacksmith: false,
        }
    }

    fn options(end_seed_exclusive: u64, max_results: usize) -> SearchOptions {
        SearchOptions {
            start_seed: 0,
            end_seed_exclusive,
            workers: NonZeroUsize::MIN,
            chunk_size: NonZeroUsize::new(4).unwrap(),
            max_results: NonZeroUsize::new(max_results).unwrap(),
        }
    }

    fn wait_until_finished(session: &NativeSession) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !session.search.is_finished() {
            assert!(
                Instant::now() < deadline,
                "native test search did not finish"
            );
            std::thread::yield_now();
        }
    }

    fn packet_count(packet: &[u8]) -> usize {
        assert_eq!(&packet[..4], b"SSR1");
        usize::from(u16::from_be_bytes([packet[4], packet[5]]))
    }

    #[test]
    fn polling_drains_terminal_results_before_reporting_completion() {
        let generator = Arc::new(MatchingGenerator);
        let session = NativeSession::start(&generator, query(), options(16, 32)).unwrap();
        wait_until_finished(&session);

        let status = session.status();
        assert_eq!(status, [STATE_RUNNING, 16, 16, ERROR_NONE]);

        let mut drained = 0;
        while drained < 16 {
            let packet = session.poll(3).unwrap();
            let count = packet_count(&packet);
            assert!(count <= 3);
            drained += count;
        }
        assert_eq!(session.poll(3).unwrap(), b"SSR1\0\0");
        assert_eq!(session.status(), [STATE_COMPLETED, 16, 16, ERROR_NONE]);
    }

    #[test]
    fn cancellation_is_cooperative_and_has_no_error_code() {
        let entered = Arc::new(Gate::default());
        let release = Arc::new(Gate::default());
        let generator = Arc::new(GatedGenerator {
            entered: Arc::clone(&entered),
            release: Arc::clone(&release),
        });
        let session = NativeSession::start(&generator, query(), options(64, 64)).unwrap();

        entered.wait_until_open();
        session.cancel();
        session.cancel();
        release.open();
        wait_until_finished(&session);

        assert_eq!(session.status(), [STATE_CANCELLED, 0, 64, ERROR_NONE]);
        assert_eq!(session.poll(8).unwrap(), b"SSR1\0\0");
    }

    #[test]
    fn a_worker_panic_has_a_stable_failure_code() {
        let generator = Arc::new(PanickingGenerator);
        let session = NativeSession::start(&generator, query(), options(4, 4)).unwrap();
        wait_until_finished(&session);

        let status = session.status();
        assert_eq!(status[0], STATE_FAILED);
        assert_eq!(status[1], 0);
        assert_eq!(status[2], 4);
        assert_eq!(status[3], ERROR_SEARCH_WORKER_FAILED);
        assert_ne!(status[3], ERROR_NONE);
        let diagnostic = worker_failure_diagnostic(&session.search).unwrap();
        assert!(diagnostic.contains("0 (AAA-AAA-AAA)..=3 (AAA-AAA-AAD)"));
        assert!(diagnostic.contains("intentional worker failure"));
    }

    #[test]
    fn registry_close_removes_first_and_is_idempotent() {
        let generator = Arc::new(MatchingGenerator);
        let session = NativeSession::start(&generator, query(), options(4, 4)).unwrap();
        let registry = SessionRegistry::new();
        let handle = registry.insert(session);

        assert_ne!(handle, 0);
        assert_eq!(registry.len(), 1);
        assert!(registry.get(handle).is_some());
        assert!(close_session(&registry, handle));
        assert_eq!(registry.len(), 0);
        assert!(!close_session(&registry, handle));
    }

    #[test]
    fn scout_helper_validates_then_generates_one_depth_twenty_four_world() {
        let generator = RecordingScoutGenerator::default();
        let packet = scout_seed_packet(&generator, b"abc-def-ghi").unwrap();
        let seed = DungeonSeed::from_code("ABC-DEF-GHI").unwrap();

        assert_eq!(generator.calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            *generator
                .inputs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            vec![(seed, 24)]
        );
        assert_eq!(decode_scout_world(&packet).unwrap(), matching_world(seed));
    }

    #[test]
    fn scout_helper_rejects_bad_input_without_running_generation() {
        let generator = RecordingScoutGenerator::default();
        assert_eq!(
            scout_seed_packet(&generator, b"AAA-AAA-AA0"),
            Err(ScoutPacketError::Request(WireError::InvalidSeedCode))
        );
        assert_eq!(
            scout_seed_packet(&generator, &[0xff]),
            Err(ScoutPacketError::Request(WireError::InvalidUtf8))
        );
        assert_eq!(generator.calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn protected_scout_helper_contains_generator_panics() {
        assert_eq!(
            protected_scout_seed_packet(&PanickingGenerator, b"AAA-AAA-AAA"),
            Err(ScoutCallError::Panicked)
        );
    }
}
