/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('api', {
    // Get/set the current directory
    current_buck_dir: () => ipcRenderer.invoke('current-buck-dir'),
    select_buck_dir: () => ipcRenderer.invoke('select-buck-dir'),

    // Run yak <action>
    status: () => ipcRenderer.invoke('buck2-status'),
    targets: (target, host) => ipcRenderer.invoke('buck2-targets', target, host),
    attributes: (target, host) => ipcRenderer.invoke('buck2-attributes', target, host),
    providers: (target, host) => ipcRenderer.invoke('buck2-providers', target, host),
});
