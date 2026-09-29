/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.installer.android;

import dev.yak.android.AdbHelper;
import dev.yak.android.IsolatedApkInfo;
import dev.yak.android.exopackage.AndroidDeviceInfo;
import dev.yak.android.exopackage.ExopackageInstaller;
import dev.yak.android.exopackage.IsolatedExopackageInfo;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.installer.InstallId;
import dev.yak.installer.InstallResult;
import java.time.Duration;
import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.logging.Level;
import java.util.logging.Logger; // NOPMD

/** Installs an Android Apk */
class AndroidInstall {
  private static final Logger LOG = Logger.getLogger(AndroidInstall.class.getName());
  private static final DateTimeFormatter INSTALL_COMPLETION_TIME_FORMAT =
      DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss");

  private final IsolatedApkInfo apkInfo;
  private final Optional<IsolatedExopackageInfo> exopackageInfo;
  private final AbsPath rootPath;
  private final InstallId installId;
  private final AndroidCommandLineOptions cliOptions;
  private final boolean installViaSd = false;
  private final Logger logger;
  private final AdbHelper adbHelper;
  private final InstallState state;

  public AndroidInstall(
      Logger logger,
      AbsPath rootPath,
      AndroidCommandLineOptions cliOptions,
      IsolatedApkInfo apkInfo,
      Optional<IsolatedExopackageInfo> exopackageInfo,
      InstallId installId,
      InstallState state,
      AdbHelper adbHelper) {
    this.logger = logger;
    this.rootPath = rootPath;
    this.apkInfo = apkInfo;
    this.exopackageInfo = exopackageInfo;
    this.installId = installId;
    this.cliOptions = cliOptions;
    this.state = state;
    this.adbHelper = adbHelper;
  }

  /** Uses AdbHelper to do actual install with APK */
  public synchronized InstallResult installApk() {
    List<Map<String, String>> deviceInfos = new ArrayList();
    try {
      if (cliOptions.cleanUp) {
        adbHelper.uninstallApp(state.packageName(), cliOptions.keepUserData);
      } else {
        if (cliOptions.uninstallFirst) {
          adbHelper.uninstallApp(state.packageName(), cliOptions.keepUserData);
        }
        logger.info(String.format("Attempting install of %s", apkInfo.getApkPath()));
        Instant start = Instant.now();
        // Everything from here on is device work, including probing it for its properties.
        long deviceWorkStart = System.currentTimeMillis();

        boolean isExopackage = exopackageInfo.map(ExopackageInstaller::isExopackage).orElse(false);
        Set<AndroidDeviceInfo> androidDeviceInfos = adbHelper.getAndroidDeviceInfo(apkInfo);
        for (AndroidDeviceInfo deviceInfo : androidDeviceInfos) {
          Map<String, String> infoMap = new LinkedHashMap<>();
          infoMap.put("sdk", deviceInfo.getSdk());
          infoMap.put("abi", deviceInfo.getAbi());
          infoMap.put("locale", deviceInfo.getLocale());
          infoMap.put("build_fingerprint", deviceInfo.getBuildFingerprint());
          infoMap.put("is_emulator", deviceInfo.isEmulator() ? "1" : "0");
          infoMap.put("density", deviceInfo.getDensity().toString());
          infoMap.put("transport", deviceInfo.getTransport());
          // With the device fields rather than the timings below, so that it is still reported
          // when an install fails before any timing is complete.
          infoMap.put("is_exopackage", isExopackage ? "1" : "0");
          deviceInfos.add(infoMap);
        }
        for (AndroidDeviceInfo deviceInfo : androidDeviceInfos) {
          adbHelper.throwIfIncompatibleAbi(deviceInfo, apkInfo);
        }

        adbHelper.installApk(
            apkInfo,
            exopackageInfo,
            rootPath,
            installViaSd,
            /* quiet= */ false,
            installId.getValue(),
            state.packageName());
        state.metrics().recordDeviceWork(deviceWorkStart, System.currentTimeMillis());

        // Only now are the stage timings complete, so the metrics cannot be gathered any earlier.
        Map<String, String> installMetrics =
            state.metrics().summarise(System.currentTimeMillis(), state.artifacts().arrivals());
        deviceInfos.forEach(infoMap -> infoMap.putAll(installMetrics));
        Instant completedAt = Instant.now();
        logger.info(
            formatCompletionMessage(
                apkInfo.getApkPath().getFileName().toString(),
                start,
                completedAt,
                ZoneId.systemDefault()));

        String packageName = state.packageName();

        if (cliOptions.enableAppLinks) {
          try {
            adbHelper.adbCall(
                "enable app links",
                (device) -> {
                  device.enableAppLinks(packageName);
                  return true;
                },
                true);
          } catch (Exception e) {
            logger.warning("Failed to enable app links: " + e.getMessage());
          }
        }

        if (cliOptions.run || cliOptions.activity != null || cliOptions.intentUri != null) {
          adbHelper.startActivityForIsolatedApk(
              apkInfo,
              installId.getValue(),
              cliOptions.activity,
              cliOptions.intentUri,
              cliOptions.waitForDebugger,
              cliOptions.skipSetDebugApp);
        }
      }
      return new InstallResult(deviceInfos, Optional.empty());
    } catch (AndroidInstallException exc) {
      return new InstallResult(deviceInfos, Optional.of(exc.getInstallError()));
    } catch (Exception err) {
      String errMsg = err.getMessage();
      logger.log(
          Level.SEVERE,
          String.format("Error while installing %s. Error message: %s", installId, errMsg),
          err);
      return new InstallResult(
          deviceInfos,
          Optional.of(errMsg).map(AndroidInstallErrorClassifier.INSTANCE::fromErrorMessage));
    }
  }

  static String formatCompletionMessage(
      String apkName, Instant start, Instant completedAt, ZoneId zoneId) {
    return String.format(
        "Install of %s finished in %d seconds at %s",
        apkName,
        Duration.between(start, completedAt).getSeconds(),
        INSTALL_COMPLETION_TIME_FORMAT.withZone(zoneId).format(completedAt));
  }
}
