# Changelog

All notable changes to this project will be documented in this file.

This file is generated with `git-cliff` from the repository history and release tags.

## [v0.2.9] - 2026-04-09

### Bug Fixes

- **build:** Resolve blank AppImage on Intel+NVIDIA hybrid GPU systems ([`67bd326`](https://github.com/yandy-r/crosshook/commit/67bd3262c7256115c859b23f19e21147aed4bd98))

- **build:** Rename plugin-stub sentinel prefix to avoid CI mock-code check ([`157b884`](https://github.com/yandy-r/crosshook/commit/157b884f943b82688dbef5ea78e7f864d5fba86d))

### Features

- **ui:** Standardize route banners and launch hero layout ([#164](https://github.com/yandy-r/crosshook/issues/164)) ([`0cf831d`](https://github.com/yandy-r/crosshook/commit/0cf831d3642fa46ccd41e1253c35dbb6a80f45fb))

- **ui:** Rebalance profile wizard with full field parity ([#161](https://github.com/yandy-r/crosshook/issues/161)) ([#166](https://github.com/yandy-r/crosshook/issues/166)) ([`1865d60`](https://github.com/yandy-r/crosshook/commit/1865d608c3914507913438de95b9fc8e0b4051c9))

- **ui:** Install Game flow parity with wizard (phase 3) ([#167](https://github.com/yandy-r/crosshook/issues/167)) ([`6f6380a`](https://github.com/yandy-r/crosshook/commit/6f6380aac6ea471497223b451d7b9f8a4a075685))

- **install:** Run EXE/MSI ad-hoc launcher under Setup tab (UI standardization phase 4) ([#168](https://github.com/yandy-r/crosshook/issues/168)) ([`7d7cbbd`](https://github.com/yandy-r/crosshook/commit/7d7cbbd4dd64127e29d6d6a276008f8b98a212a6))

- **build:** Expand browser mock coverage across all domains (Phase 2) ([#173](https://github.com/yandy-r/crosshook/issues/173)) ([`78a38ba`](https://github.com/yandy-r/crosshook/commit/78a38baea1fa71469ccce5d46dadc971400d75e9))

- **frontend:** Extract IPC into hooks for launch dep gate and profile verify ([#176](https://github.com/yandy-r/crosshook/issues/176)) ([`622cf31`](https://github.com/yandy-r/crosshook/commit/622cf3117f3bb5cf49a0f9ade7f8ead26c5996bc))

- **core:** Profile collections backend foundation (Phase 1) ([#182](https://github.com/yandy-r/crosshook/issues/182)) ([`0fefc0c`](https://github.com/yandy-r/crosshook/commit/0fefc0c2804c2eb6aa63ca2fd2fa6e6326c9e32b))

- **ui:** Profile collections sidebar, view modal, shared state ([#183](https://github.com/yandy-r/crosshook/issues/183)) ([`a8ead0c`](https://github.com/yandy-r/crosshook/commit/a8ead0cd37d67ce534a450ac48280db9db58633f))

- **ui:** Per-collection launch defaults — Phase 3 ([#179](https://github.com/yandy-r/crosshook/issues/179)) ([#184](https://github.com/yandy-r/crosshook/issues/184)) ([`51e1347`](https://github.com/yandy-r/crosshook/commit/51e13475b782655a98024fcdd470d3e70879e8f3))

- **collections:** Preset TOML import/export and dev modals ([#185](https://github.com/yandy-r/crosshook/issues/185)) ([`f1a5f1f`](https://github.com/yandy-r/crosshook/commit/f1a5f1f0dd263e1217ce912ecb274588fcdf5803))

- **ui:** Profile collections polish, integration tests, Steam Deck validation — Phase 5 ([#181](https://github.com/yandy-r/crosshook/issues/181)) ([#186](https://github.com/yandy-r/crosshook/issues/186)) ([`842e70a`](https://github.com/yandy-r/crosshook/commit/842e70aa757f007ccfecd461144e1a46af7c250a))

## [v0.2.8] - 2026-04-07

### Bug Fixes

- **discovery:** Harden external search and trainer-source validation ([`6cdf96d`](https://github.com/yandy-r/crosshook/commit/6cdf96d4c72ad33b771f40eefd1b5d7beeed515e))

### Features

- **ui:** Add library game details modal ([#152](https://github.com/yandy-r/crosshook/issues/152)) ([`94fa99d`](https://github.com/yandy-r/crosshook/commit/94fa99d898c9c80b06abf8cf0c0358d404400964))

- Add prefix storage health monitoring and cleanup tools ([#153](https://github.com/yandy-r/crosshook/issues/153)) ([`239f707`](https://github.com/yandy-r/crosshook/commit/239f7076690d895f574761acde9ab50020b58130))

- **security:** Network isolation for trainers via unshare --net ([#154](https://github.com/yandy-r/crosshook/issues/154)) ([`b317590`](https://github.com/yandy-r/crosshook/commit/b317590c3eacd6d4d34f7a233ae90dab5d6ee91c))

- **protondb:** Community-driven config suggestions with catalog matching ([#155](https://github.com/yandy-r/crosshook/issues/155)) ([`21a548f`](https://github.com/yandy-r/crosshook/commit/21a548f4bda3f0ae953191e827bbc15c137a12f3))

- **security:** Trainer executable SHA-256 verification at launch ([#156](https://github.com/yandy-r/crosshook/issues/156)) ([`b2e93fc`](https://github.com/yandy-r/crosshook/commit/b2e93fcc71681316d25ce0d4d5f5ec6e43d79fad))

- **discovery:** Trainer discovery Phase A — tap-local search and UI ([#67](https://github.com/yandy-r/crosshook/issues/67)) ([`060d869`](https://github.com/yandy-r/crosshook/commit/060d869a8995e9734817403e4b78b3e25d29866d))

- **community:** Enhance tap URL validation to accept bare absolute paths ([`7f8f196`](https://github.com/yandy-r/crosshook/commit/7f8f19633f0e0d5fba463ec2d9b45360f56bf024))

- **discovery:** Trainer discovery phases A–B ([#157](https://github.com/yandy-r/crosshook/issues/157)) ([`6090ad2`](https://github.com/yandy-r/crosshook/commit/6090ad2956a6d8852ad7c1bc688a52672768e467))

- **protonup:** In-app Proton runtime management ([#159](https://github.com/yandy-r/crosshook/issues/159)) ([`4b86144`](https://github.com/yandy-r/crosshook/commit/4b86144375cc194a6ebb9a94a012cf38a9c45a9e))

## [v0.2.7] - 2026-04-04

### Bug Fixes

- **launch:** Revert umu-run preference, use direct proton for all launch paths ([`e5f182c`](https://github.com/yandy-r/crosshook/commit/e5f182cf6d2781adfa5f7ef482c42a24154a02f3))

### Features

- **launch:** Enable gamescope for steam_applaunch and trainer exports ([#145](https://github.com/yandy-r/crosshook/issues/145)) ([`8ba5b01`](https://github.com/yandy-r/crosshook/commit/8ba5b01048cf892f8981cf16ff2ecd49d652a30e))

- **profiles:** Add Proton App ID and tri-art system ([#146](https://github.com/yandy-r/crosshook/issues/146)) ([`8ee04a4`](https://github.com/yandy-r/crosshook/commit/8ee04a4fef25b7db0df00cda91b808eafd8850b3))

- **launch:** Move environment config to launch with autosave ([#147](https://github.com/yandy-r/crosshook/issues/147)) ([`620037e`](https://github.com/yandy-r/crosshook/commit/620037eb18e531482e9b9006b2be951dd4da55d4))

- **launch:** Prefer umu-run for proton helper flows ([#148](https://github.com/yandy-r/crosshook/issues/148)) ([`791b7ec`](https://github.com/yandy-r/crosshook/commit/791b7ec9ed5b92aef4222209981c02530b6ba355))

- **launch:** Detect running game processes and split launch buttons ([#149](https://github.com/yandy-r/crosshook/issues/149)) ([`1b01bf0`](https://github.com/yandy-r/crosshook/commit/1b01bf044dc74fce45d3db3bc3ad34b1ebf58113))

- **settings:** Expand app settings with profile defaults, log filter, and UI preferences ([#150](https://github.com/yandy-r/crosshook/issues/150)) ([`5e37043`](https://github.com/yandy-r/crosshook/commit/5e37043f7a116f8e919bf4930af4810ac9237e50))

- **launch:** Protontricks/winetricks integration for prefix dependencies ([#151](https://github.com/yandy-r/crosshook/issues/151)) ([`4a2ce3d`](https://github.com/yandy-r/crosshook/commit/4a2ce3d30e91b3fed39a8f1c8c602b744490c653))

## [v0.2.6] - 2026-04-02

### Bug Fixes

- **profiles:** Address ProtonDB lookup review findings ([#133](https://github.com/yandy-r/crosshook/issues/133)) ([`74087a0`](https://github.com/yandy-r/crosshook/commit/74087a058b52cc3b4ef9f9ea7a10a782f834a227))

- **ui:** Route layout, panel decor, and polish across routes ([#138](https://github.com/yandy-r/crosshook/issues/138)) ([`883b622`](https://github.com/yandy-r/crosshook/commit/883b622812dce93472c13bac7b6a9238845967a0))

- **cli:** Include mangohud in LaunchRequest from profile ([`6bb5010`](https://github.com/yandy-r/crosshook/commit/6bb5010631ea56c09f6e30430b2e4b682a809b96))

### Features

- **launch:** Gamescope wrapper integration with per-profile resolution and scaling ([#128](https://github.com/yandy-r/crosshook/issues/128)) ([`29c0934`](https://github.com/yandy-r/crosshook/commit/29c093470c73f3e0705ca45f7ccb8a711ed41d79))

- **launch:** Data-driven optimization catalog loaded from TOML instead of compiled constants ([#129](https://github.com/yandy-r/crosshook/issues/129)) ([`73ac69e`](https://github.com/yandy-r/crosshook/commit/73ac69ecabf2a4b1999f4c2d95ce2967a4a77d90))

- **profiles:** Offline-first trainer management for Steam Deck portable use ([#130](https://github.com/yandy-r/crosshook/issues/130)) ([`77e8ca0`](https://github.com/yandy-r/crosshook/commit/77e8ca01159c1e30b1dd21c9b16d619c9c7c46e3))

- **launch:** MangoHud per-profile configuration file generation ([#131](https://github.com/yandy-r/crosshook/issues/131)) ([`5992557`](https://github.com/yandy-r/crosshook/commit/59925573cd4d3e6e8d6b167a35555a47e36870e4))

- **profiles:** ProtonDB compatibility rating lookup by Steam App ID ([#132](https://github.com/yandy-r/crosshook/issues/132)) ([`2d21529`](https://github.com/yandy-r/crosshook/commit/2d215297bac4cede9fdab2dd570aec05bc294ea1))

- **profiles:** Add game metadata, cover art, and UI restructuring ([#134](https://github.com/yandy-r/crosshook/issues/134)) ([`cf07a07`](https://github.com/yandy-r/crosshook/commit/cf07a07306a5e172196a86052befbf62ffaebabd))

- **launch:** Add tabbed interface with cover art hero to Launch page ([#136](https://github.com/yandy-r/crosshook/issues/136)) ([`39c6328`](https://github.com/yandy-r/crosshook/commit/39c6328f3b91a3a94ef6639e908e5668d6450758))

- **ui:** Scroll shell, install/update heroes, and compact launch optimizations ([#137](https://github.com/yandy-r/crosshook/issues/137)) ([`ba63297`](https://github.com/yandy-r/crosshook/commit/ba63297aab98be44970ed32f1a02d61eaaecb8da))

- **ui:** Add Library Home page with poster art grid ([#139](https://github.com/yandy-r/crosshook/issues/139)) ([`1aff9c0`](https://github.com/yandy-r/crosshook/commit/1aff9c08f64195103f50b33e0d9583d0502cdbdb))

## [v0.2.5] - 2026-03-31

### Bug Fixes

- **community:** Add A6 bounds for version fields and pinned commit hex validation ([`0f2b96b`](https://github.com/yandy-r/crosshook/commit/0f2b96bca49cc2cb46debbd84c32fec421cd6469))

- **version:** Correct steam_applaunch version tracking and launch outcome classification ([`11baba2`](https://github.com/yandy-r/crosshook/commit/11baba227bda448bedfc2dfa835b709a2956b1ae))

- **version:** Address PR review findings for version correlation ([`e4eefcd`](https://github.com/yandy-r/crosshook/commit/e4eefcdf78f990d6a0a747ffad8c80efcfdd8780))

- **branchlet:** Update terminal command from 'zed .' to 'cursor .' in .branchlet.json ([`5687913`](https://github.com/yandy-r/crosshook/commit/56879136d67887d8ece51157845307b6756aa887))

- **branchlet:** Update terminal command in .branchlet.json to use GUI editor ([`e0ece59`](https://github.com/yandy-r/crosshook/commit/e0ece592fe73c54766af83de2ca3c5c62c057a6e))

### Documentation

- **ref:** Add feature specification and research documents for CLI command wiring ([`08bb988`](https://github.com/yandy-r/crosshook/commit/08bb9885b0d92e07e82f54034271d467475e5925))

### Features

- **ui:** Add pinned profiles for quick launch on Profiles and Launch pages ([#113](https://github.com/yandy-r/crosshook/issues/113)) ([`d345c7a`](https://github.com/yandy-r/crosshook/commit/d345c7a9471ab20051794e73706fddb889df3b63))

- **profile:** Add portable/local override layers ([#114](https://github.com/yandy-r/crosshook/issues/114)) ([`252a3fd`](https://github.com/yandy-r/crosshook/commit/252a3fd757570ee30a455795d9600bce4c1ee98a))

- **profile:** Add Proton version migration tool for stale path detection and replacement ([#115](https://github.com/yandy-r/crosshook/issues/115)) ([`b0d1747`](https://github.com/yandy-r/crosshook/commit/b0d1747e23ca2a929de0a8b57f0b93f7e174558e))

- **community:** Add guided import wizard and live profile sync ([#116](https://github.com/yandy-r/crosshook/issues/116)) ([`2f639dd`](https://github.com/yandy-r/crosshook/commit/2f639dd11865713fa5a258a46c9b4969fe591a34))

- **metadata:** Add version snapshots schema, store, and steam manifest extension ([`e6fb130`](https://github.com/yandy-r/crosshook/commit/e6fb130ee6285712ee91823976cd4e8482475ce3))

- **launch:** Named optimization presets per profile ([#121](https://github.com/yandy-r/crosshook/issues/121)) ([`aad839d`](https://github.com/yandy-r/crosshook/commit/aad839dc3bc8a3eda6ee33da7485ccf491ab571b))

- **version:** Integrate version tracking into launch, startup, and health pipelines ([`748d85e`](https://github.com/yandy-r/crosshook/commit/748d85e362c91885b0fc17e016247b2fb27d7bca))

- **version:** Add Phase 3 UX — version dashboard, warnings, and launch state persistence ([`8cd976b`](https://github.com/yandy-r/crosshook/commit/8cd976be2786dbac07eab9cc194b21ab7bb0854e))

- **profiles:** Configuration history with diff and rollback ([#124](https://github.com/yandy-r/crosshook/issues/124)) ([`9f9212c`](https://github.com/yandy-r/crosshook/commit/9f9212ccda63c04683863c8ec6b9ea2c197d6da2))

- **onboarding:** Add trainer onboarding wizard with guided profile creation ([#125](https://github.com/yandy-r/crosshook/issues/125)) ([`007ce95`](https://github.com/yandy-r/crosshook/commit/007ce956434c18aa3840a0f001524da8b74ee25a))

- **profiles:** Support custom env vars across launch flows ([#126](https://github.com/yandy-r/crosshook/issues/126)) ([`16a8e7f`](https://github.com/yandy-r/crosshook/commit/16a8e7f4b4347298c77cdf638e64233d0f609dff))

- **cli:** Wire all placeholder commands to crosshook-core and extend launch support ([#127](https://github.com/yandy-r/crosshook/issues/127)) ([`2cba5f1`](https://github.com/yandy-r/crosshook/commit/2cba5f1d4be8f48dfbdadac05b8e1625c5a8db30))

## [v0.2.4] - 2026-03-29

### Bug Fixes

- **launcher:** Populate profile_id when exporting launchers ([`a4c7238`](https://github.com/yandy-r/crosshook/commit/a4c7238a2d3e0fb86628bc40305fe916d61fff0f))

- **metadata:** Enhance community tap indexing with transactional UPSERT and migration improvements ([`d6ad04b`](https://github.com/yandy-r/crosshook/commit/d6ad04b0d81648863f058b645311503afb22afa2))

- **security:** Enforce baseline Tauri CSP ([`e9f739e`](https://github.com/yandy-r/crosshook/commit/e9f739e5fac51895841505322e94ec2fc807f682))

- **ui:** Smooth launcher preview modal scrolling ([#97](https://github.com/yandy-r/crosshook/issues/97)) ([`3123cdd`](https://github.com/yandy-r/crosshook/commit/3123cdd243d569725674c554048ca286306b075f))

### Documentation

- **profile-health:** Revise health dashboard spec and integrate SQLite metadata layer ([`ed0bd03`](https://github.com/yandy-r/crosshook/commit/ed0bd03d853061be8750ff7347346d52fa5b720b))

- **health:** Add Health Dashboard Page for profile diagnostics ([`5aca99e`](https://github.com/yandy-r/crosshook/commit/5aca99e7c05a8c7f01e82d73b6308613e6239f66))

- **health:** Add implementation plan for Health Dashboard Page ([`70ae03e`](https://github.com/yandy-r/crosshook/commit/70ae03e9ac6730c410e019d46008b9997d0030b9))

### Features

- **launch:** Add dry run / preview launch mode ([#86](https://github.com/yandy-r/crosshook/issues/86)) ([`bb60051`](https://github.com/yandy-r/crosshook/commit/bb6005174259fd2ddf30b61799eb6e23d14599bf))

- **launch:** Add post-launch failure diagnostics ([`96b7275`](https://github.com/yandy-r/crosshook/commit/96b7275aabf6eebaa499108421856d84a25fe719))

- **launch:** Enhance launch state management with helper log path tracking ([`7964f1e`](https://github.com/yandy-r/crosshook/commit/7964f1e8c87a1144e04805f9f6b415151ba27481))

- **metadata:** Add SQLite metadata layer for stable profile identity (Phase 1) ([`aaeb603`](https://github.com/yandy-r/crosshook/commit/aaeb603f6853897f4a69ca85391c9d3fcaeababe))

- **metadata:** Add operational history tracking for launches and launchers (Phase 2) ([`b5e62a5`](https://github.com/yandy-r/crosshook/commit/b5e62a5cc41d2a376cd2089a615d3d9050b0c670))

- **metadata:** Add community catalog, collections, cache, and usage insights (Phase 3) ([`d525f1d`](https://github.com/yandy-r/crosshook/commit/d525f1daad19e1d81664ab43760b781dd68c3e88))

- **health:** Add profile health dashboard MVP ([`ed73b30`](https://github.com/yandy-r/crosshook/commit/ed73b30131783188dc23c512db21ac5bb125b1a6))

- **health:** Metadata enrichment + startup integration (Phases B & C) ([#98](https://github.com/yandy-r/crosshook/issues/98)) ([`efa1855`](https://github.com/yandy-r/crosshook/commit/efa1855749ab69296641df5ad2e732b54e617405))

- **health:** Health snapshot persistence + trend analysis (Phase D) ([#100](https://github.com/yandy-r/crosshook/issues/100)) ([`84e8f0c`](https://github.com/yandy-r/crosshook/commit/84e8f0cbc06bef92bf68f8863a78eac78b5b618f))

- **health:** Add Health Dashboard page with profile diagnostics ([#104](https://github.com/yandy-r/crosshook/issues/104)) ([`e2eda59`](https://github.com/yandy-r/crosshook/commit/e2eda599dfd1b42ea0cdb81a3dce58dbd3868846))

- **health:** Expand dashboard table with sortable metadata columns ([#105](https://github.com/yandy-r/crosshook/issues/105)) ([`c3fa496`](https://github.com/yandy-r/crosshook/commit/c3fa4965285365ead1058d8a3a9bcde82b92bacb))

- **export:** Detect stale launchers in manager ([#106](https://github.com/yandy-r/crosshook/issues/106)) ([`9b1592a`](https://github.com/yandy-r/crosshook/commit/9b1592af84cebc31a046704f80c864eeb467db71))

- **community:** Export shareable profiles from GUI ([#107](https://github.com/yandy-r/crosshook/issues/107)) ([`956f86c`](https://github.com/yandy-r/crosshook/commit/956f86c54dcaf276e55c2cfbe14371cc5cba121f))

- **ui:** Adaptive Deck layout ([#54](https://github.com/yandy-r/crosshook/issues/54)) ([#108](https://github.com/yandy-r/crosshook/issues/108)) ([`63a3cad`](https://github.com/yandy-r/crosshook/commit/63a3cadba8c13ae5bf6da885e6dfd430275630a5))

- **launch:** Extend optimization catalog and vendor options UI ([#109](https://github.com/yandy-r/crosshook/issues/109)) ([`6ce0e38`](https://github.com/yandy-r/crosshook/commit/6ce0e389a857c491f0f938fe376249dbe988bedc))

- **community:** Add tap commit pinning and pin/unpin UI ([#110](https://github.com/yandy-r/crosshook/issues/110)) ([`cf27cff`](https://github.com/yandy-r/crosshook/commit/cf27cff95ea103bbac18fb17c47c98bc1c7a02ce))

- **diagnostics:** Add diagnostic bundle export ([`01dd2ac`](https://github.com/yandy-r/crosshook/commit/01dd2ac339f734b3b55a00d7681d30fc8b692625))

## [v0.2.3] - 2026-03-27

### Bug Fixes

- **launch:** Add actionable validation guidance and reset page scroll ([#79](https://github.com/yandy-r/crosshook/issues/79)) ([`98fdf18`](https://github.com/yandy-r/crosshook/commit/98fdf1840dc09ca86d4a0dacc66e28827e047b64))

### Features

- **settings:** Add new commands for repository and echo output in settings.local.json ([`2c51e83`](https://github.com/yandy-r/crosshook/commit/2c51e83346d45e6865dcb3907c1e00c4ce430209))

- **docs:** Add comprehensive research reports on emerging trends and additional features for CrossHook ([`2004d11`](https://github.com/yandy-r/crosshook/commit/2004d115705ef770626d6080200f652f7cec4423))

- **update:** Add update game panel for applying patches to Proton prefixes ([#81](https://github.com/yandy-r/crosshook/issues/81)) ([`bd090b5`](https://github.com/yandy-r/crosshook/commit/bd090b580a88ddf46b7045ed977005f3fdf11d3d))

- **settings:** Add new Bash commands for grep and npx vite in settings.local.json ([`c7851fe`](https://github.com/yandy-r/crosshook/commit/c7851fe75bfdc6b396daab3f1bb20e89b3631d83))

- **ui:** Add collapsible sections to all pages for easier navigation ([`240d619`](https://github.com/yandy-r/crosshook/commit/240d61912cfecc3f689adf3cfff4958f838ed882))

- **profile:** Add profile duplication with unique name generation ([#82](https://github.com/yandy-r/crosshook/issues/82)) ([`c60e784`](https://github.com/yandy-r/crosshook/commit/c60e7846e9c99a29cb8a45f10e9aaa1d58b60cfe))

- **profile:** Add rename with overwrite protection and launcher cascade ([#83](https://github.com/yandy-r/crosshook/issues/83)) ([`5866808`](https://github.com/yandy-r/crosshook/commit/5866808e3a4d0c4f2067b447593783f28b87f0e8))

## [v0.2.2] - 2026-03-26

### Bug Fixes

- **release:** Enforce changelog hygiene ([`6f66f4b`](https://github.com/yandy-r/crosshook/commit/6f66f4b93658d4f87ddd7632e7a66d60e2764b16))

- **ui:** Show launcher icon for proton profiles ([`d4f5113`](https://github.com/yandy-r/crosshook/commit/d4f51134da0dcba60ce17e627f72b5e2b76c6674))

- **ui:** Proton install fallback path and arrow-key scroll override ([`c242301`](https://github.com/yandy-r/crosshook/commit/c242301471710c2684f3f381c179c778844e8678))

- **ui:** Resolve 9 review issues from PR #34 ([`9268b63`](https://github.com/yandy-r/crosshook/commit/9268b637bc86d7adcf956bbfc8fdb90384599a97))

- **ui:** Resolve 15 suggestion-level review items from PR #34 ([`61e0a53`](https://github.com/yandy-r/crosshook/commit/61e0a53f1930319cb8fa5ee2d7a7ae4fe9bfa495))

- **ui:** Adjust padding and border styles for content areas ([`1ab02da`](https://github.com/yandy-r/crosshook/commit/1ab02da9f3a07f55b9daaf6952633a496370f5ba))

### Features

- **profile:** Add install review modal flow ([#29](https://github.com/yandy-r/crosshook/issues/29)) ([`62a8f08`](https://github.com/yandy-r/crosshook/commit/62a8f08d585ebb6ed5c862b3647cc04213c4ea22))

- **launch:** Add proton-run launch optimizations ([#31](https://github.com/yandy-r/crosshook/issues/31)) ([`86f74f0`](https://github.com/yandy-r/crosshook/commit/86f74f0f96a1622ff7670119a41cf9a9b3286cde))

- **ui:** Sidebar navigation, page banners, themed selects, and console drawer ([#33](https://github.com/yandy-r/crosshook/issues/33)) ([`36d2579`](https://github.com/yandy-r/crosshook/commit/36d25792062ef9d77568ba33a0a98445b97915f2))

- **settings:** Add command to grep for specific symbols in TypeScript files ([`5b64467`](https://github.com/yandy-r/crosshook/commit/5b64467d84c010b1e09484832ce787b9ca940e4f))

- **launch:** Add per-profile trainer loading modes ([#35](https://github.com/yandy-r/crosshook/issues/35)) ([`86949dd`](https://github.com/yandy-r/crosshook/commit/86949dd8dd5c22a617cd82c4b4a6355c4b639b3d))

## [v0.2.1] - 2026-03-25

### Bug Fixes

- **release:** Update AppImage upload step to use specific asset path ([`0751e87`](https://github.com/yandy-r/crosshook/commit/0751e87d42bfbe405dc3912670fc77676b877647))

- **ui:** Keep launch panel anchored while logs stream ([#27](https://github.com/yandy-r/crosshook/issues/27)) ([`d5bedcf`](https://github.com/yandy-r/crosshook/commit/d5bedcf7537c14ab573124cc10a2464a2c8b1676))

- **release:** Restore and validate native workspace manifest ([`6bd2589`](https://github.com/yandy-r/crosshook/commit/6bd25893d972f7b95dc0fbc21b3b1ccfca8e1fa7))

### Features

- **native:** Implement install game workflow ([#23](https://github.com/yandy-r/crosshook/issues/23)) ([`704c478`](https://github.com/yandy-r/crosshook/commit/704c4780f32b721d1c72f0b25149bebcf3cb5517))

- **launcher:** Implement launcher lifecycle management ([#25](https://github.com/yandy-r/crosshook/issues/25)) ([`fa51a74`](https://github.com/yandy-r/crosshook/commit/fa51a74f22762749ba2de7c6375050102c3fa62c))

## [v0.2.0] - 2026-03-23

### Bug Fixes

- **native:** Align build and release versioning ([`6624afa`](https://github.com/yandy-r/crosshook/commit/6624afaca4b408b9383ecd294d0e4d72f55ce1ca))

- **native:** Restore workspace release manifest ([`9275fce`](https://github.com/yandy-r/crosshook/commit/9275fceaeed943e525422147b00a680eba0dfebe))

### Features

- Implement the platform-native-ui native app feature set ([#20](https://github.com/yandy-r/crosshook/issues/20)) ([`84242d4`](https://github.com/yandy-r/crosshook/commit/84242d482a399f19abb0cefe5a42bc82dfd7ba7a))

## [v0.1.1] - 2026-03-23

### Features

- Add comprehensive documentation for platform-native Linux UI ([`5e6720a`](https://github.com/yandy-r/crosshook/commit/5e6720a4678d0ccc62ebdc1c29ef0ca048016405))

- Expand platform-native UI documentation and enhance local settings ([`cae6261`](https://github.com/yandy-r/crosshook/commit/cae6261fd4c5af79addce801669fc559618dee47))

- Expand platform-native UI analysis and documentation ([`ca1bc92`](https://github.com/yandy-r/crosshook/commit/ca1bc927e7b0e620e83d26ec9efcf91f5e4e2a5f))

## [v0.1.0] - 2026-03-19

<!-- generated by git-cliff -->
