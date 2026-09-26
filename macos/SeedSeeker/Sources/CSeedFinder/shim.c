#include "CSeedFinder.h"
#ifdef SEEDSEEKER_IOS
#include "ios_engine_revision.h"
#else
#include "engine_revision.h"
#endif

// Changing the archive changes this object, forcing SwiftPM to relink clients.
const char seedseeker_engine_archive_sha256[] = SEEDSEEKER_ENGINE_ARCHIVE_SHA256;
