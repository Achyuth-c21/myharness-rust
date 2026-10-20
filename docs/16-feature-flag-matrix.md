# Appendix A — Runtime feature-flag matrix

**Evidence:** `evidence/runtime-api-surface.json`, captured from the live renderer through CDP. **Verified.**

This is the complete set of 131 feature gates exposed by the running renderer. “Status” is the runtime gate result, not a product promise. A supported gate can still be hidden by another configuration condition. The companion counts are 64 supported, 51 unavailable, and 16 unsupported.

`unavailable` means the gate is known to the renderer but not available in this environment/build. `unsupported` carries a reason code. Reasons are reproduced verbatim (no secrets).

## Supported (64)

| Flag | Status | Reason / code |
|---|---|---|
| `launch` | supported |  |
| `quickEntryGlobalShortcut` | supported |  |
| `trayIcon` | supported |  |
| `customQuickEntryDictationShortcut` | supported |  |
| `ultrareviewErrorCategories` | supported |  |
| `spawnTaskPendingPop` | supported |  |
| `spawnTaskInFlightIds` | supported |  |
| `folderWorktrees` | supported |  |
| `folderWorktreesRemote` | supported |  |
| `folderWorktreesListedDiscard` | supported |  |
| `chillingSlothFeat` | supported |  |
| `chillingSlothSshShell` | supported |  |
| `terminalShellReported` | supported |  |
| `chillingSlothSshWorktreeLocation` | supported |  |
| `chillingSlothRemoteFilePreview` | supported |  |
| `chillingSlothEnterprise` | supported |  |
| `chillingSlothLocal` | supported |  |
| `keepAwake` | supported |  |
| `codeSessionKeepAwake` | supported |  |
| `ccdLandingTools` | supported |  |
| `ccdMcpVerbsMidTurn` | supported |  |
| `ccdMcpTurnOnSshHost` | supported |  |
| `ccdRestartRemoteClaudeCode` | supported |  |
| `ccdSshStartsWithoutBlockedPlugins` | supported |  |
| `openWithConsentAnyLanding` | supported |  |
| `desktopTopBar` | supported |  |
| `ccdPlugins` | supported |  |
| `ccdPluginSourceRouting` | supported |  |
| `computerUseComplianceGate` | supported |  |
| `sessionFolderFileAccess` | supported |  |
| `conversationFolderGrants` | supported |  |
| `localSessionsWithoutGit` | supported |  |
| `coworkArtifactPopout` | supported |  |
| `sidePanePopoutNativeViews` | supported |  |
| `simulatorNativeViewPopout` | supported |  |
| `simulatorPopoutShortcuts` | supported |  |
| `simulatorNativeViewPerPane` | supported |  |
| `popoutZoom` | supported |  |
| `popoutTitleBarOverlay` | supported |  |
| `filePreviewNativeViewRouting` | supported |  |
| `claudeDesignWindow` | supported |  |
| `coworkRemoteSessionSpaces` | supported |  |
| `bootPlaceholder` | supported |  |
| `coworkBranchSession` | supported |  |
| `coworkScheduledTaskProjects` | supported |  |
| `coworkLocalSessionProjects` | supported |  |
| `coworkScheduledTaskRepoint` | supported |  |
| `sshAttachmentTransport` | supported |  |
| `coworkSeededSummon` | supported |  |
| `seededKind` | supported |  |
| `sendNowEscalation` | supported |  |
| `steerDetachField` | supported |  |
| `sessionMentions` | supported |  |
| `inlinePastes` | supported |  |
| `turnDiscardedEvent` | supported |  |
| `coworkThinkingInSend` | supported |  |
| `bootstrapConfig` | supported |  |
| `chatIn3p` | supported |  |
| `mcpPersistentAlwaysAllow` | supported |  |
| `mcpScheduledTaskAlwaysAllow` | supported |  |
| `epitaxyMcpApps` | supported |  |
| `sideSessionProposalPicks` | supported |  |
| `sideSessionProposalWorktreePick` | supported |  |
| `sideSessionOfferCloudStart` | supported |  |

## Unavailable (51)

| Flag | Status | Reason / code |
|---|---|---|
| `artifactsPane` | unavailable |  |
| `nativeQuickEntry` | unavailable |  |
| `quickEntryDictation` | unavailable |  |
| `sableOtter` | unavailable |  |
| `telescopeBridge` | unavailable |  |
| `mcpFinishInApp` | unavailable |  |
| `copperLarkA` | unavailable |  |
| `copperLarkB` | unavailable |  |
| `copperLarkC` | unavailable |  |
| `copperLarkD` | unavailable |  |
| `quietPenguin` | unavailable |  |
| `rubberDuck` | unavailable |  |
| `chillingSlothSshPuttyKeyConversion` | unavailable |  |
| `wakeScheduler` | unavailable |  |
| `ccdPromptSuggestionsSetting` | unavailable |  |
| `coworkWatchers` | unavailable |  |
| `coworkKappa` | unavailable |  |
| `coworkArtifacts` | unavailable |  |
| `trustedFolderSettingsConsent` | unavailable |  |
| `coworkAutoModeAlwaysAllowOverride` | unavailable |  |
| `framebufferPreview` | unavailable |  |
| `iosSimulatorDuo` | unavailable |  |
| `simulatorScreenshotEditor` | unavailable |  |
| `simulatorMiniView` | unavailable |  |
| `grandPrix` | unavailable |  |
| `ccdGitEngine` | unavailable |  |
| `sessionPrOwnership` | unavailable |  |
| `spaceMemoryBridge` | unavailable |  |
| `multiAccount` | unavailable |  |
| `grandPrixRequest` | unavailable |  |
| `wslForkSession` | unavailable |  |
| `wslTerminal` | unavailable |  |
| `wslMcp` | unavailable |  |
| `wslPreview` | unavailable |  |
| `wslLocalFiles` | unavailable |  |
| `wslRoutines` | unavailable |  |
| `humbleSky` | unavailable |  |
| `violinBow` | unavailable |  |
| `violinBowHomeSettings` | unavailable |  |
| `chatTab` | unavailable |  |
| `selfHostedSessions` | unavailable |  |
| `selfHostedCowork` | unavailable |  |
| `desktopHome` | unavailable |  |
| `chatCodeExecution` | unavailable |  |
| `mcpScheduledTaskApprovalLifetime` | unavailable |  |
| `sideSessions` | unavailable |  |
| `btwChildSessions` | unavailable |  |
| `nestedArchiveKeepsFamily` | unavailable |  |
| `epitaxyMcpAppPane` | unavailable |  |
| `ccdConnectorTools` | unavailable |  |
| `ccdTeleportSnapshot` | unavailable |  |

## Unsupported (16)

| Flag | Status | Reason / code |
|---|---|---|
| `coworkBrowser` | unsupported | cowork_browser_disabled; code=unknown |
| `yukonSilver` | unsupported | Checking virtualization support…; code=unknown |
| `yukonSilverGems` | unsupported | Checking virtualization support…; code=unknown |
| `yukonSilverGemsCache` | unsupported | Checking virtualization support…; code=unknown |
| `computerUse` | unsupported | Computer use is not yet enabled for this account; code=unknown |
| `computerUseRollout` | unsupported | Computer use is not yet enabled for this account; code=not_enabled |
| `computerUseAppScoped` | unsupported | Background app-scoped mode is macOS-only; code=unsupported_platform |
| `computerUsePerAppAccess` | unsupported | A Wayland session shares the whole screen; code=unknown |
| `coworkWatchRecord` | unsupported | Watch-record is not available on this platform; code=unsupported_platform |
| `coworkCopperHeron` | unsupported | Watch-record is not available on this platform; code=unsupported_platform |
| `iosSimulator` | unsupported | iOS Simulator panel requires macOS; code=unsupported_platform |
| `androidEmulator` | unsupported | Android emulator is disabled by its rollout flag; code=unknown |
| `iosSimulatorH264` | unsupported | iOS Simulator panel requires macOS; code=unsupported_platform |
| `iosSimulatorAccessibilityReader` | unsupported | iOS Simulator panel requires macOS; code=unsupported_platform |
| `chromeExtensionInstall` | unsupported | No per-user Chrome extension install mechanism; code=unsupported_platform |
| `tearOffHalo` | unsupported | Tear-off halo overlay requires macOS; code=unsupported_platform |
