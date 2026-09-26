// SPDX-License-Identifier: GPL-3.0-or-later
import BackgroundTasks
import SeedSeekerKit
import UIKit

/// Keeps a user-started search attached to iOS's continued-processing activity.
/// When the system cannot grant execution time, the controller drains native
/// workers and saves its checkpoint before suspension instead.
@MainActor
final class BackgroundSearch {
    private static weak var registeredOwner: BackgroundSearch?
    private static var registrationSucceeded: Bool?

    private let controller: SearchController
    private let identifierPrefix: String
    private var requestIdentifier: String?
    private var continuedTask: BGContinuedProcessingTask?
    private var submission: Task<Void, Never>?
    private var monitoring: Task<Void, Never>?
    private var graceTask: UIBackgroundTaskIdentifier = .invalid
    private var isForeground = true

    init(controller: SearchController) {
        self.controller = controller
        identifierPrefix = (Bundle.main.bundleIdentifier ?? "dev.seedseeker.ios") + ".search."
        register()
    }

    /// Call after SearchController.start(), while the user is in the app.
    func start() {
        guard controller.isRunning, requestIdentifier == nil, isForeground else { return }

        // BackgroundTasks is unsupported on Simulator. Its lifecycle still
        // exercises the same durable interruption and foreground recovery.
        #if targetEnvironment(simulator)
        return
        #else
        guard Self.registrationSucceeded == true else { return }
        let identifier = identifierPrefix + UUID().uuidString
        requestIdentifier = identifier
        let title = progressTitle
        let subtitle = progressSubtitle
        submission = Task { [weak self] in
            let submitted = await Self.submit(identifier: identifier, title: title, subtitle: subtitle)
            guard let self else {
                BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: identifier)
                return
            }
            guard self.requestIdentifier == identifier else {
                // Submission can return after Stop or a background transition.
                BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: identifier)
                return
            }
            self.submission = nil
            if !submitted {
                self.finish(success: false)
                if !self.isForeground { self.pauseForSuspension() }
            }
        }
        observeSearch(identifier: identifier)
        #endif
    }

    /// Both the app's Stop button and the system activity's stop control use
    /// the same explicit cancellation, which must not resume automatically.
    func stop() {
        controller.cancel()
        guard let identifier = requestIdentifier else { return }
        BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: identifier)
        // Leave the execution assertion alive until the native workers drain.
        // The observation loop then completes the activity and releases it.
    }

    func didEnterBackground() {
        isForeground = false
        guard controller.isRunning, continuedTask == nil else { return }
        finish(success: false)
        pauseForSuspension()
    }

    func didBecomeActive() {
        isForeground = true
        endGraceTask()
        controller.resumeInterrupted()
        start()
    }

    private func register() {
        #if !targetEnvironment(simulator)
        Self.registeredOwner = self
        guard Self.registrationSucceeded == nil else { return }
        Self.registrationSucceeded = BGTaskScheduler.shared.register(
            forTaskWithIdentifier: identifierPrefix + "*", using: .main
        ) { task in
            // BGTaskScheduler promises this handler runs on the supplied queue.
            MainActor.assumeIsolated {
                guard let continued = task as? BGContinuedProcessingTask,
                      let owner = Self.registeredOwner else {
                    task.setTaskCompleted(success: false)
                    return
                }
                owner.accept(continued)
            }
        }
        #endif
    }

    // The iOS 27 SDK requires submission away from the main thread; its async
    // API also reports errors that the previous synchronous API could miss.
    @concurrent
    private static func submit(identifier: String, title: String, subtitle: String) async -> Bool {
        let request = BGContinuedProcessingTaskRequest(identifier: identifier, title: title, subtitle: subtitle)
        request.strategy = .fail
        do {
            try await BGTaskScheduler.shared.submitTaskRequest(request)
            return true
        } catch {
            return false
        }
    }

    private func accept(_ task: BGContinuedProcessingTask) {
        guard task.identifier == requestIdentifier, controller.isRunning else {
            task.setTaskCompleted(success: false)
            return
        }
        continuedTask = task
        task.expirationHandler = { [weak self] in
            Task { @MainActor in self?.expire(identifier: task.identifier) }
        }
        task.progress.isCancellable = true
        task.progress.cancellationHandler = { [weak self] in
            Task { @MainActor in self?.stop() }
        }
        updateProgress(task)
    }

    private func observeSearch(identifier: String) {
        monitoring?.cancel()
        monitoring = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, self.requestIdentifier == identifier else { return }
                guard self.controller.isRunning else {
                    self.finish(success: !self.controller.hasPendingSearch && self.controller.state != .failed)
                    return
                }
                if let task = self.continuedTask { self.updateProgress(task) }
                do { try await Task.sleep(for: .seconds(1)) }
                catch { return }
            }
        }
    }

    private var progressTitle: String {
        controller.refineProgress == nil ? "Searching for seeds" : "Refining seeds"
    }

    private var progressSubtitle: String {
        if let progress = controller.refineProgress {
            return "\(progress.checked) of \(progress.total) saved seeds checked"
        }
        return "\(controller.foundCount) found · \(controller.scannedSeeds) checked"
    }

    private func updateProgress(_ task: BGContinuedProcessingTask) {
        task.updateTitle(progressTitle, subtitle: progressSubtitle)
        if let progress = controller.refineProgress {
            task.progress.totalUnitCount = Int64(max(1, progress.total))
            task.progress.completedUnitCount = Int64(progress.checked)
        } else {
            task.progress.totalUnitCount = max(1, controller.totalSeeds)
            task.progress.completedUnitCount = min(controller.scannedSeeds, task.progress.totalUnitCount)
        }
    }

    private func expire(identifier: String) {
        guard identifier == requestIdentifier, let task = continuedTask else { return }
        let wasCancelled = task.progress.isCancelled
        continuedTask = nil
        task.progress.cancellationHandler = nil
        task.expirationHandler = nil
        finish(success: false)

        // A foreground search does not need a background execution assertion.
        // Releasing an expired assertion must not interrupt that search; an
        // explicit cancellation from the system activity still stops it.
        if isForeground && !wasCancelled {
            task.setTaskCompleted(success: false)
            return
        }

        // In the background, keep the expiring activity until the final atomic
        // checkpoint is on disk. Periodic checkpoints also cover abrupt death.
        if wasCancelled { controller.cancel() }
        else { controller.interrupt() }
        Task { [weak self, controller] in
            await controller.waitUntilSettled()
            task.setTaskCompleted(success: false)
            // The app may have returned while workers were draining. The
            // controller's pending checkpoint is cleared by any explicit Stop,
            // so resumeInterrupted() cannot revive a cancelled search.
            if !wasCancelled, let self, self.isForeground {
                controller.resumeInterrupted()
                self.start()
            }
        }
    }

    private func pauseForSuspension() {
        guard controller.isRunning else { return }
        if graceTask == .invalid {
            graceTask = UIApplication.shared.beginBackgroundTask(withName: "Searching for seeds") { [weak self] in
                Task { @MainActor in self?.endGraceTask() }
            }
        }
        let assertion = graceTask
        controller.interrupt()
        Task { [weak self, controller] in
            await controller.waitUntilSettled()
            guard let self, self.graceTask == assertion else { return }
            self.endGraceTask()
        }
    }

    private func finish(success: Bool) {
        submission?.cancel(); submission = nil
        monitoring?.cancel(); monitoring = nil
        if let identifier = requestIdentifier {
            BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: identifier)
        }
        requestIdentifier = nil
        if let task = continuedTask {
            task.progress.cancellationHandler = nil
            task.expirationHandler = nil
            if success { task.progress.completedUnitCount = task.progress.totalUnitCount }
            task.setTaskCompleted(success: success)
            continuedTask = nil
        }
    }

    private func endGraceTask() {
        guard graceTask != .invalid else { return }
        let assertion = graceTask
        graceTask = .invalid
        UIApplication.shared.endBackgroundTask(assertion)
    }
}
