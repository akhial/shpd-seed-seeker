// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import org.junit.runners.model.FrameworkMethod
import org.robolectric.RobolectricTestRunner
import org.robolectric.internal.bytecode.InstrumentationConfiguration

/** The JVM can load a native library once; share the JNI entry points with the engine tests. */
class ScoutRobolectricTestRunner(testClass: Class<*>) : RobolectricTestRunner(testClass) {
    override fun createClassLoaderConfig(method: FrameworkMethod): InstrumentationConfiguration =
        InstrumentationConfiguration.Builder(super.createClassLoaderConfig(method))
            .doNotAcquireClass("dev.seedseeker.app.engine.JniBindings")
            .build()
}
