# Changelog

All notable changes to this project will be documented in this file.

This file is generated with `git-cliff` from the repository history and release tags.
## [v0.2.7] - 2026-04-04


### Bug Fixes

- **launch:** Revert umu-run preference, use direct proton for all launch paths ([`7894353`](https://github.com/yandy-r/crosshook/commit/78943537733b389c7406cb91e0aec802308fe86a))


### Features

- **launch:** Enable gamescope for steam_applaunch and trainer exports ([#145](https://github.com/yandy-r/crosshook/issues/145)) ([`e5362c4`](https://github.com/yandy-r/crosshook/commit/e5362c48772df3529b0871cddca1bd6e98f86d3b))

- **profiles:** Add Proton App ID and tri-art system ([#146](https://github.com/yandy-r/crosshook/issues/146)) ([`cd37b3a`](https://github.com/yandy-r/crosshook/commit/cd37b3aa35a977c5a231f880795ae5037e2dbfd9))

- **launch:** Move environment config to launch with autosave ([#147](https://github.com/yandy-r/crosshook/issues/147)) ([`3f9e2c5`](https://github.com/yandy-r/crosshook/commit/3f9e2c5e3ad67c5f25fd9aa9a9d61ab77f6e3e24))

- **launch:** Prefer umu-run for proton helper flows ([#148](https://github.com/yandy-r/crosshook/issues/148)) ([`e31264f`](https://github.com/yandy-r/crosshook/commit/e31264fb08fe102f671e199f05f80ee0bc05e4a0))

- **launch:** Detect running game processes and split launch buttons ([#149](https://github.com/yandy-r/crosshook/issues/149)) ([`9003fab`](https://github.com/yandy-r/crosshook/commit/9003fab31a956975cc369572c6b900ce3055ed41))

- **settings:** Expand app settings with profile defaults, log filter, and UI preferences ([#150](https://github.com/yandy-r/crosshook/issues/150)) ([`fee9b36`](https://github.com/yandy-r/crosshook/commit/fee9b36513a1b0a232c7a18a9d1b9836addbcadb))

- **launch:** Protontricks/winetricks integration for prefix dependencies ([#151](https://github.com/yandy-r/crosshook/issues/151)) ([`5049761`](https://github.com/yandy-r/crosshook/commit/50497618c291dde40fe4a9022686bcbed925e860))


## [v0.2.6] - 2026-04-02


### Bug Fixes

- **profiles:** Address ProtonDB lookup review findings ([#133](https://github.com/yandy-r/crosshook/issues/133)) ([`b745f38`](https://github.com/yandy-r/crosshook/commit/b745f38affad52c4c82f778bbb578445e76c7a3d))

- **ui:** Route layout, panel decor, and polish across routes ([#138](https://github.com/yandy-r/crosshook/issues/138)) ([`d9c145a`](https://github.com/yandy-r/crosshook/commit/d9c145aa1e8d6a152e014163e67c3bccd363c054))

- **cli:** Include mangohud in LaunchRequest from profile ([`9a821e5`](https://github.com/yandy-r/crosshook/commit/9a821e5b5522bbe3cfa09c7b1cd0181278f7c779))


### Features

- **launch:** Gamescope wrapper integration with per-profile resolution and scaling ([#128](https://github.com/yandy-r/crosshook/issues/128)) ([`7dcab83`](https://github.com/yandy-r/crosshook/commit/7dcab83a6c68cfcd21fac0a032b902de7c955ff9))

- **launch:** Data-driven optimization catalog loaded from TOML instead of compiled constants ([#129](https://github.com/yandy-r/crosshook/issues/129)) ([`c558f56`](https://github.com/yandy-r/crosshook/commit/c558f56f94e35123ce932bfbad3ce4761b7b46fc))

- **profiles:** Offline-first trainer management for Steam Deck portable use ([#130](https://github.com/yandy-r/crosshook/issues/130)) ([`6d1ca2b`](https://github.com/yandy-r/crosshook/commit/6d1ca2b0a5332f401c08dab20fbd83372968bbde))

- **launch:** MangoHud per-profile configuration file generation ([#131](https://github.com/yandy-r/crosshook/issues/131)) ([`473cdee`](https://github.com/yandy-r/crosshook/commit/473cdeeafef9b0d6f7336714e5108058cf3b202f))

- **profiles:** ProtonDB compatibility rating lookup by Steam App ID ([#132](https://github.com/yandy-r/crosshook/issues/132)) ([`df2915f`](https://github.com/yandy-r/crosshook/commit/df2915f7e34d8844c7376e8e950b74c2668a6fc5))

- **profiles:** Add game metadata, cover art, and UI restructuring ([#134](https://github.com/yandy-r/crosshook/issues/134)) ([`4a6cc6c`](https://github.com/yandy-r/crosshook/commit/4a6cc6cc12e298101b1e68103621ef0fa8fd678e))

- **launch:** Add tabbed interface with cover art hero to Launch page ([#136](https://github.com/yandy-r/crosshook/issues/136)) ([`ca0360e`](https://github.com/yandy-r/crosshook/commit/ca0360e17f9f1119c57e6265f5a70f808ab08574))

- **ui:** Scroll shell, install/update heroes, and compact launch optimizations ([#137](https://github.com/yandy-r/crosshook/issues/137)) ([`84775e5`](https://github.com/yandy-r/crosshook/commit/84775e53a04c9e2761908a3d8c3466d0152294d0))

- **ui:** Add Library Home page with poster art grid ([#139](https://github.com/yandy-r/crosshook/issues/139)) ([`5f463f8`](https://github.com/yandy-r/crosshook/commit/5f463f80bbc830707ce3768d933f704c09b9d452))


## [v0.2.5] - 2026-03-31


### Bug Fixes

- **community:** Add A6 bounds for version fields and pinned commit hex validation ([`586d280`](https://github.com/yandy-r/crosshook/commit/586d28064003826d4f0b8379d2e190403f370d7f))

- **version:** Correct steam_applaunch version tracking and launch outcome classification ([`fc968d7`](https://github.com/yandy-r/crosshook/commit/fc968d7cf92e9c6ed18652339ec804849937bf5c))

- **version:** Address PR review findings for version correlation ([`714c3a0`](https://github.com/yandy-r/crosshook/commit/714c3a0c4c99d752558d3ddc997bb7fa30350596))

- **branchlet:** Update terminal command from 'zed .' to 'cursor .' in .branchlet.json ([`b80a3d9`](https://github.com/yandy-r/crosshook/commit/b80a3d9c02a0d1959cd7c9d81fa790dba00342b9))

- **branchlet:** Update terminal command in .branchlet.json to use GUI editor ([`fde3d0a`](https://github.com/yandy-r/crosshook/commit/fde3d0a6a78aac3d2dc2479554620917e9340551))


### Documentation

- **ref:** Add feature specification and research documents for CLI command wiring ([`d0c084b`](https://github.com/yandy-r/crosshook/commit/d0c084b96d937025f5435c18ee0fea8d9fa4a7a9))


### Features

- **ui:** Add pinned profiles for quick launch on Profiles and Launch pages ([#113](https://github.com/yandy-r/crosshook/issues/113)) ([`7091937`](https://github.com/yandy-r/crosshook/commit/7091937c97a9d1323557bf10042fbb29251e51e2))

- **profile:** Add portable/local override layers ([#114](https://github.com/yandy-r/crosshook/issues/114)) ([`2ae0ea0`](https://github.com/yandy-r/crosshook/commit/2ae0ea0c6fe6eead39ad402daf469eb02c1cb67e))

- **profile:** Add Proton version migration tool for stale path detection and replacement ([#115](https://github.com/yandy-r/crosshook/issues/115)) ([`834fd76`](https://github.com/yandy-r/crosshook/commit/834fd76b1e3185538c5d070aa79b193ab61eda9f))

- **community:** Add guided import wizard and live profile sync ([#116](https://github.com/yandy-r/crosshook/issues/116)) ([`d93f8cd`](https://github.com/yandy-r/crosshook/commit/d93f8cddb1941528bae9bff05cb0b4da93273c64))

- **metadata:** Add version snapshots schema, store, and steam manifest extension ([`45e9782`](https://github.com/yandy-r/crosshook/commit/45e9782d9e45aecd1c066cf06e8cf06c59631bae))

- **launch:** Named optimization presets per profile ([#121](https://github.com/yandy-r/crosshook/issues/121)) ([`6855dc9`](https://github.com/yandy-r/crosshook/commit/6855dc9c580fdea70e56a4d9924d59e592a32193))

- **version:** Integrate version tracking into launch, startup, and health pipelines ([`7c58196`](https://github.com/yandy-r/crosshook/commit/7c58196303b5134b79a515ef58b9ecf8da607fdb))

- **version:** Add Phase 3 UX — version dashboard, warnings, and launch state persistence ([`0f07f42`](https://github.com/yandy-r/crosshook/commit/0f07f42c5aae23b4ca82aa06085d7c62396199a9))

- **profiles:** Configuration history with diff and rollback ([#124](https://github.com/yandy-r/crosshook/issues/124)) ([`79d3509`](https://github.com/yandy-r/crosshook/commit/79d3509ff5efb975d8d7148c6a0d124e79643d32))

- **onboarding:** Add trainer onboarding wizard with guided profile creation ([#125](https://github.com/yandy-r/crosshook/issues/125)) ([`71c5215`](https://github.com/yandy-r/crosshook/commit/71c52151b21186d2594e9313dcfb7066041f44da))

- **profiles:** Support custom env vars across launch flows ([#126](https://github.com/yandy-r/crosshook/issues/126)) ([`2850e19`](https://github.com/yandy-r/crosshook/commit/2850e19814477a6ba66ef8ba39376684241b5d69))

- **cli:** Wire all placeholder commands to crosshook-core and extend launch support ([#127](https://github.com/yandy-r/crosshook/issues/127)) ([`14e2305`](https://github.com/yandy-r/crosshook/commit/14e2305389891b47c8186df606a09df7f8227221))


## [v0.2.4] - 2026-03-29


### Bug Fixes

- **launcher:** Populate profile_id when exporting launchers ([`5a6602b`](https://github.com/yandy-r/crosshook/commit/5a6602b23d40bc00588a1644856d33b57d10dfb0))

- **metadata:** Enhance community tap indexing with transactional UPSERT and migration improvements ([`252e65b`](https://github.com/yandy-r/crosshook/commit/252e65b53091c6f26d6e8ad13d3c6e4c07bde8fe))

- **security:** Enforce baseline Tauri CSP ([`7810d58`](https://github.com/yandy-r/crosshook/commit/7810d58754671e193b3490322bdbd6ac338120f0))

- **ui:** Smooth launcher preview modal scrolling ([#97](https://github.com/yandy-r/crosshook/issues/97)) ([`984537e`](https://github.com/yandy-r/crosshook/commit/984537ee03ad4dcf7e46e87aa01e99094655a28a))


### Documentation

- **profile-health:** Revise health dashboard spec and integrate SQLite metadata layer ([`2294f20`](https://github.com/yandy-r/crosshook/commit/2294f20ada1333351d6575c563599e3f63ebe02e))

- **health:** Add Health Dashboard Page for profile diagnostics ([`10f35bc`](https://github.com/yandy-r/crosshook/commit/10f35bcdf0147405def4cae99d1ce23e86342d7b))

- **health:** Add implementation plan for Health Dashboard Page ([`b9150ec`](https://github.com/yandy-r/crosshook/commit/b9150ecb6bdc18f89ddceb9eec2298e697f86fb9))


### Features

- **launch:** Add dry run / preview launch mode ([#86](https://github.com/yandy-r/crosshook/issues/86)) ([`f29e891`](https://github.com/yandy-r/crosshook/commit/f29e891448537adbdb3b5093d73e3ff1e7f0e8f6))

- **launch:** Add post-launch failure diagnostics ([`82e3187`](https://github.com/yandy-r/crosshook/commit/82e31874505cb818e50bb8b0df7e73f490cdfaa8))

- **launch:** Enhance launch state management with helper log path tracking ([`7a62b65`](https://github.com/yandy-r/crosshook/commit/7a62b65c9cf682ae387fd2e4d46e49b29c2be559))

- **metadata:** Add SQLite metadata layer for stable profile identity (Phase 1) ([`062ac1f`](https://github.com/yandy-r/crosshook/commit/062ac1f8ef478fe8fcd44508e40918b6cae64067))

- **metadata:** Add operational history tracking for launches and launchers (Phase 2) ([`2d8d6b7`](https://github.com/yandy-r/crosshook/commit/2d8d6b724e1463bbcd1ae6bb484ae52b163027cd))

- **metadata:** Add community catalog, collections, cache, and usage insights (Phase 3) ([`fffe10f`](https://github.com/yandy-r/crosshook/commit/fffe10f7472f512db0f8466fdc9f065b9fe5e7a1))

- **health:** Add profile health dashboard MVP ([`779766c`](https://github.com/yandy-r/crosshook/commit/779766c3f087599e355d295c41d344c59de62945))

- **health:** Metadata enrichment + startup integration (Phases B & C) ([#98](https://github.com/yandy-r/crosshook/issues/98)) ([`3b9901c`](https://github.com/yandy-r/crosshook/commit/3b9901cea6af37b0431532e1ef3ed47e4956ef69))

- **health:** Health snapshot persistence + trend analysis (Phase D) ([#100](https://github.com/yandy-r/crosshook/issues/100)) ([`125e6b1`](https://github.com/yandy-r/crosshook/commit/125e6b149e2532549c217b95dead2feafcf9d069))

- **health:** Add Health Dashboard page with profile diagnostics ([#104](https://github.com/yandy-r/crosshook/issues/104)) ([`8fd4b44`](https://github.com/yandy-r/crosshook/commit/8fd4b444dd31e1dea28994d0b4eba94ee1c4f0fc))

- **health:** Expand dashboard table with sortable metadata columns ([#105](https://github.com/yandy-r/crosshook/issues/105)) ([`8d1386a`](https://github.com/yandy-r/crosshook/commit/8d1386a5fde55f183fcb786bba1ca1712567af63))

- **export:** Detect stale launchers in manager ([#106](https://github.com/yandy-r/crosshook/issues/106)) ([`52d5e44`](https://github.com/yandy-r/crosshook/commit/52d5e448303ae71ea4d5ca6d5237b6706964ca95))

- **community:** Export shareable profiles from GUI ([#107](https://github.com/yandy-r/crosshook/issues/107)) ([`740c7cd`](https://github.com/yandy-r/crosshook/commit/740c7cd20f9355b6dd5babf2a1a6481f1d7505bd))

- **ui:** Adaptive Deck layout ([#54](https://github.com/yandy-r/crosshook/issues/54)) ([#108](https://github.com/yandy-r/crosshook/issues/108)) ([`efa0d5e`](https://github.com/yandy-r/crosshook/commit/efa0d5ee373b5b71bec97f6959934ca2029b02c0))

- **launch:** Extend optimization catalog and vendor options UI ([#109](https://github.com/yandy-r/crosshook/issues/109)) ([`20f6b27`](https://github.com/yandy-r/crosshook/commit/20f6b27d519a77f669c0f850664e681840a740e9))

- **community:** Add tap commit pinning and pin/unpin UI ([#110](https://github.com/yandy-r/crosshook/issues/110)) ([`cc6fa44`](https://github.com/yandy-r/crosshook/commit/cc6fa440c1950d8e30c4f2aee5f46becafc397e4))

- **diagnostics:** Add diagnostic bundle export ([`f7bd08b`](https://github.com/yandy-r/crosshook/commit/f7bd08b8945b073a72baf8e2eecbfc4768ab1cbd))


## [v0.2.3] - 2026-03-27


### Bug Fixes

- **launch:** Add actionable validation guidance and reset page scroll ([#79](https://github.com/yandy-r/crosshook/issues/79)) ([`f6b71bc`](https://github.com/yandy-r/crosshook/commit/f6b71bcaae398fa7b48fbcf76119a1bc9f940fd3))


### Features

- **settings:** Add new commands for repository and echo output in settings.local.json ([`311dd47`](https://github.com/yandy-r/crosshook/commit/311dd47ac9edbf793bb9136270d74d43686a25e1))

- **docs:** Add comprehensive research reports on emerging trends and additional features for CrossHook ([`d8961e1`](https://github.com/yandy-r/crosshook/commit/d8961e115cf011023f9cf9a10e8c7aac0698c889))

- **update:** Add update game panel for applying patches to Proton prefixes ([#81](https://github.com/yandy-r/crosshook/issues/81)) ([`0980723`](https://github.com/yandy-r/crosshook/commit/0980723b3d20a5b5e2e9d7574c273da29c8c89a3))

- **settings:** Add new Bash commands for grep and npx vite in settings.local.json ([`61f6ffc`](https://github.com/yandy-r/crosshook/commit/61f6ffcb8bfdf3df082d899dec1707306084698b))

- **ui:** Add collapsible sections to all pages for easier navigation ([`79cba3c`](https://github.com/yandy-r/crosshook/commit/79cba3cf6dd97f776c92cf21a18d8265ed9b5b2c))

- **profile:** Add profile duplication with unique name generation ([#82](https://github.com/yandy-r/crosshook/issues/82)) ([`fbd5325`](https://github.com/yandy-r/crosshook/commit/fbd5325284083e493804fbda39e1d03debd62ea3))

- **profile:** Add rename with overwrite protection and launcher cascade ([#83](https://github.com/yandy-r/crosshook/issues/83)) ([`23cebd8`](https://github.com/yandy-r/crosshook/commit/23cebd80b62f165140ab7c0c9128260adc379d9e))


## [v0.2.2] - 2026-03-26


### Bug Fixes

- **release:** Enforce changelog hygiene ([`a6d40ea`](https://github.com/yandy-r/crosshook/commit/a6d40ea1f794f94df7b6bc599dfe86aa0d819f38))

- **ui:** Show launcher icon for proton profiles ([`6d62be4`](https://github.com/yandy-r/crosshook/commit/6d62be4f57ff561f6d90117fe87a7a684fcc375a))

- **ui:** Proton install fallback path and arrow-key scroll override ([`cc369d3`](https://github.com/yandy-r/crosshook/commit/cc369d39c014c45a847313e4811c913b8ad3f6bc))

- **ui:** Resolve 9 review issues from PR #34 ([`9de17a2`](https://github.com/yandy-r/crosshook/commit/9de17a2127cd9aa56a27a5256a15ec8cd371f664))

- **ui:** Resolve 15 suggestion-level review items from PR #34 ([`451ce0c`](https://github.com/yandy-r/crosshook/commit/451ce0c6f4be1fe161690ffc734d965a08111b73))

- **ui:** Adjust padding and border styles for content areas ([`c3bc395`](https://github.com/yandy-r/crosshook/commit/c3bc39549fc85b55fa4d8b3a0af61e1cd2481863))


### Features

- **profile:** Add install review modal flow ([#29](https://github.com/yandy-r/crosshook/issues/29)) ([`3e1261f`](https://github.com/yandy-r/crosshook/commit/3e1261fefe026ff0caf86b984fc7f5a12cc959cd))

- **launch:** Add proton-run launch optimizations ([#31](https://github.com/yandy-r/crosshook/issues/31)) ([`41db070`](https://github.com/yandy-r/crosshook/commit/41db0702e07c34c9ab9d9612eb487d4071422022))

- **ui:** Sidebar navigation, page banners, themed selects, and console drawer ([#33](https://github.com/yandy-r/crosshook/issues/33)) ([`b326bfd`](https://github.com/yandy-r/crosshook/commit/b326bfddb0fbd7cdceb7e51b8d0d812e685916df))

- **settings:** Add command to grep for specific symbols in TypeScript files ([`d453eb5`](https://github.com/yandy-r/crosshook/commit/d453eb5ffab3e7761fdcd31439b74494f95ad2bf))

- **launch:** Add per-profile trainer loading modes ([#35](https://github.com/yandy-r/crosshook/issues/35)) ([`4d04041`](https://github.com/yandy-r/crosshook/commit/4d04041621c05f9e002d680d5a79c5c013847acf))


## [v0.2.1] - 2026-03-25


### Bug Fixes

- **release:** Update AppImage upload step to use specific asset path ([`dd6b5bb`](https://github.com/yandy-r/crosshook/commit/dd6b5bbaa43278fc1f66e0542be9edab147f7ffd))

- **ui:** Keep launch panel anchored while logs stream ([#27](https://github.com/yandy-r/crosshook/issues/27)) ([`746672a`](https://github.com/yandy-r/crosshook/commit/746672aaf6428fa5b7d0db7e2aa41da46f727013))

- **release:** Restore and validate native workspace manifest ([`7432cbb`](https://github.com/yandy-r/crosshook/commit/7432cbbc68b412e4fdb519609683d644b53be962))


### Features

- **native:** Implement install game workflow ([#23](https://github.com/yandy-r/crosshook/issues/23)) ([`97fc609`](https://github.com/yandy-r/crosshook/commit/97fc60901a77ab853a19cdc10de71a7c816cfdf0))

- **launcher:** Implement launcher lifecycle management ([#25](https://github.com/yandy-r/crosshook/issues/25)) ([`28b6beb`](https://github.com/yandy-r/crosshook/commit/28b6beb45a6bf57bd8a3003f9f76b25e4750a316))


## [v0.2.0] - 2026-03-23


### Bug Fixes

- **native:** Align build and release versioning ([`bbd9ed9`](https://github.com/yandy-r/crosshook/commit/bbd9ed9cd98a551c9c0b5a73e7ead0ab65aa0813))

- **native:** Restore workspace release manifest ([`dccd475`](https://github.com/yandy-r/crosshook/commit/dccd475b659c5fe48c7d29e74d44d58abfd0a609))


### Features

- Implement the platform-native-ui native app feature set ([#20](https://github.com/yandy-r/crosshook/issues/20)) ([`246a5ea`](https://github.com/yandy-r/crosshook/commit/246a5ea374ab9606e9a51d79642a69e624fa8926))


## [v0.1.1] - 2026-03-23


### Features

- Add comprehensive documentation for platform-native Linux UI ([`266864e`](https://github.com/yandy-r/crosshook/commit/266864e9a3f80a9fb1bd5827492a4188a929ea86))

- Expand platform-native UI documentation and enhance local settings ([`9ed9016`](https://github.com/yandy-r/crosshook/commit/9ed90162a882506104ad3a9d3b31db6e1bdb53b1))

- Expand platform-native UI analysis and documentation ([`b88bd7c`](https://github.com/yandy-r/crosshook/commit/b88bd7c4d784733cf6d6759dd2c1dfc7adfbfc06))


## [v0.1.0] - 2026-03-19


### Release

- Remove controller support and TV mode, streamline executable name ([`dafbae5`](https://github.com/yandy-r/crosshook/commit/dafbae55e7a925294313206ec5f7df760a6423e5))


## [v5.0] - 2025-04-04


### Features

- Rename executable to choochoo.exe ([`2d953fb`](https://github.com/yandy-r/crosshook/commit/2d953fb661a86e519181d4b5b4d7316267562a88))


<!-- generated by git-cliff -->
