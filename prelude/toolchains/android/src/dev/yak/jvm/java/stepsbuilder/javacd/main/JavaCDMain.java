/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.java.stepsbuilder.javacd.main;

import com.google.common.collect.ImmutableMap;
import dev.yak.core.util.log.Logger;
import dev.yak.jvm.cd.CompilerDaemonLoggerUtil;
import dev.yak.jvm.cd.CompilerDaemonRunner;
import dev.yak.jvm.cd.ErrorInterceptor;
import java.io.IOException;
import java.util.logging.Level;

/**
 * JavaCD main class.
 *
 * <p>This provides a simple executable that can run any of the javacd actions.
 */
public class JavaCDMain {
  private static final String LOG_PATH = "yak-out/v2/javacd";

  /** Main entrypoint of JavaCD worker tool. */
  public static void main(String[] args) throws IOException {
    try {
      JavaCDCommand command = new JavaCDCommand(args, ImmutableMap.copyOf(System.getenv()));
      CompilerDaemonLoggerUtil.setDefaultLogger("javacd_worker", LOG_PATH);
      CompilerDaemonLoggerUtil.setConsoleHandlerLogLevelTo(Level.WARNING);
      Logger logger = Logger.get(JavaCDMain.class.getName());
      System.setErr(new ErrorInterceptor());
      Runtime.getRuntime()
          .addShutdownHook(
              new Thread(
                  () -> {
                    System.out.flush();
                    System.err.flush();
                  }));
      CompilerDaemonRunner.run(command);
      logger.info(String.format("Starting JavaCDWorkerTool %s", command));
      command.postExecute();
      System.err.println("JavaCDWorkerTool succeeded!");
      System.exit(0);
    } catch (Exception e) {
      System.err.println("JavaCDWorkerTool failed: " + e.getMessage());
    }
    System.exit(2);
  }
}
