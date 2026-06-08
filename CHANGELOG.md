# Changelog

All notable changes to this project will be documented in this file.

This file is generated with `git-cliff` from the repository history and release tags.

## [v0.4.1] - 2026-06-08

### Bug Fixes

- **launch:** Allow Steam hook optimizations ([`2d662a6`](https://github.com/yandy-r/crosshook/commit/2d662a64c2380d211fe291bc3fd506606bc49022))

## [v0.4.0] - 2026-06-08

### Documentation

- **roadmap:** Refresh post-release priorities ([`9e713b6`](https://github.com/yandy-r/crosshook/commit/9e713b6d8f7b725155f86827ea561c22b39bc3b0))

- **roadmap:** Add upstream cross-links for glib advisory ([#26](https://github.com/yandy-r/crosshook/issues/26)) ([`82f0f9e`](https://github.com/yandy-r/crosshook/commit/82f0f9e0f2ed2a764de56d2e58164b0e3fbc8a12))

- **roadmap:** Move Do next to top and refresh priorities ([`b40069c`](https://github.com/yandy-r/crosshook/commit/b40069c41b0535f25c8de20585346b46bd849548))

### Features

- **library:** Add trainer tab editor ([#501](https://github.com/yandy-r/crosshook/issues/501)) ([`985e5bf`](https://github.com/yandy-r/crosshook/commit/985e5bf9e4a1192e37b61462d72f93ac787ac69f))

- **launch:** Add umu GAMEID lookup resolver ([#502](https://github.com/yandy-r/crosshook/issues/502)) ([`6474d50`](https://github.com/yandy-r/crosshook/commit/6474d50d3b88f1905b29abf93b8dbb022763dcf6))

## [v0.3.1] - 2026-06-05

### Build

- **native:** Upgrade deps for Tauri 2.11 and TypeScript 6 ([#500](https://github.com/yandy-r/crosshook/issues/500)) ([`3f56204`](https://github.com/yandy-r/crosshook/commit/3f56204ddc605dfda53d7bea3e1b1a1a0eae1d07))

## [v0.3.0] - 2026-06-05

### Bug Fixes

- **ui:** Downgrade umu coverage missing from warning to informational ([`e2b6d01`](https://github.com/yandy-r/crosshook/commit/e2b6d0115a86ff9eaba82c4af1a93aefc30cd781))

- **bug:** Fix env vars leak in CSV coverage preview tests ([#333](https://github.com/yandy-r/crosshook/issues/333)) ([`089df9f`](https://github.com/yandy-r/crosshook/commit/089df9f16224031fde471dceaf11329df0540825))

- Refactor deduplication of MANGOHUD_CONFIG insert in inject_mangohud_config_preview_env ([#334](https://github.com/yandy-r/crosshook/issues/334)) ([`5546a11`](https://github.com/yandy-r/crosshook/commit/5546a113f766623cf8365ae29be5b15ffef7d1c9))

- Refactor dedup staged-trainer path construction ([#335](https://github.com/yandy-r/crosshook/issues/335)) ([`cb6894d`](https://github.com/yandy-r/crosshook/commit/cb6894dbfc5cfcca47f26401b7cd216231e9374f))

- Refactor remove duplicate assertions in preview and profile test suites ([#337](https://github.com/yandy-r/crosshook/issues/337)) ([`4b2e352`](https://github.com/yandy-r/crosshook/commit/4b2e352d5adee0fc5356e0958b2a13a1ab496a32))

- Refactor deferred cleanups from CodeRabbit nitpicks ([#338](https://github.com/yandy-r/crosshook/issues/338)) ([`dedd53a`](https://github.com/yandy-r/crosshook/commit/dedd53af3df6d2642ed3ed9003d7172dbd81bc66))

- Invalid date rendering in SettingsPanel ([#339](https://github.com/yandy-r/crosshook/issues/339)) ([`86e6ad5`](https://github.com/yandy-r/crosshook/commit/86e6ad5109fe815762b4938c7cad1f529f4a1000))

- Refactor documentation and simplify path check in host_fs.rs ([#341](https://github.com/yandy-r/crosshook/issues/341)) ([`fe39771`](https://github.com/yandy-r/crosshook/commit/fe397718cb5ff20ad126ba25da61c8031eb36258))

- Refactor to use shared METHOD\_\* constants in resolve_launch_method ([#336](https://github.com/yandy-r/crosshook/issues/336)) ([`02f7f3e`](https://github.com/yandy-r/crosshook/commit/02f7f3e846774cae4f7a7a1e678b933f8905d392))

- Path safety in launcher_store for symlink verification ([#340](https://github.com/yandy-r/crosshook/issues/340)) ([`c9fb074`](https://github.com/yandy-r/crosshook/commit/c9fb0746869863082dda19ebe0cc40b9197afaa1))

- Refactor fallback constant in launcher naming ([#342](https://github.com/yandy-r/crosshook/issues/342)) ([`7f6e462`](https://github.com/yandy-r/crosshook/commit/7f6e46254a0e9919f97293a83079805666eaccd1))

- Validated_steam_client_install_path to accept valid Flatpak paths ([#346](https://github.com/yandy-r/crosshook/issues/346)) ([`4284cb3`](https://github.com/yandy-r/crosshook/commit/4284cb3599e54ba0d37068ca0577e5b432e47f16))

- Validation to reject control characters in profile names ([#344](https://github.com/yandy-r/crosshook/issues/344)) ([`93aa2cb`](https://github.com/yandy-r/crosshook/commit/93aa2cb746d3251db06db700888b090a623c01f2))

- Code-quality findings in profile::health from PR review ([#345](https://github.com/yandy-r/crosshook/issues/345)) ([`e756159`](https://github.com/yandy-r/crosshook/commit/e756159277e9d0ca00908f6b571db374be6e83e9))

- **scripts:** Skip generated paths in tooling ([`dd9dd0f`](https://github.com/yandy-r/crosshook/commit/dd9dd0fdc27748a688d925c9b43e3c2d53f87980))

- **ui:** Apply Phase 10 code review findings ([`0394b64`](https://github.com/yandy-r/crosshook/commit/0394b64c229639e2b71e0791ed1831a476d92f38))

- **ui:** Route wheel scroll to active ancestor ([`91af59b`](https://github.com/yandy-r/crosshook/commit/91af59b95a9faf3a357777b57975a5eb98856b97))

- **profiles:** Stabilize auto-load after refresh ([#498](https://github.com/yandy-r/crosshook/issues/498)) ([`a2e48ac`](https://github.com/yandy-r/crosshook/commit/a2e48ac0cf56d11d98999f52a8fac8787eae3faa))

- **ui:** Strip dev-mock sentinel from production bundle ([`6a41595`](https://github.com/yandy-r/crosshook/commit/6a41595a8d758c186945e5fda96f21553639516e))

### CI

- Add copilot agent setup-steps workflow and document firewall guidance ([`41ac950`](https://github.com/yandy-r/crosshook/commit/41ac950064eb59e792e2d889e91751e367e91f86))

- **lint:** Use test:coverage npm script for Vitest job ([`176913b`](https://github.com/yandy-r/crosshook/commit/176913b74cbad57102bb1c272993143f5090fb49))

- Enforce Conventional Commits on PR titles and add Copilot agent instructions ([`e3817d9`](https://github.com/yandy-r/crosshook/commit/e3817d9a96e1b22544f320fbfd6d57b9937428c4))

- Auto-strip placeholder prefixes from PR titles ([`46a2e2e`](https://github.com/yandy-r/crosshook/commit/46a2e2ea0e2d55f501cba7b4a775e0fec7361e0b))

- Normalize Conventional Commit prefix in PR title autofix ([#394](https://github.com/yandy-r/crosshook/issues/394)) ([`123df62`](https://github.com/yandy-r/crosshook/commit/123df62dec55574c301122636467ef434e2b2573))

### Documentation

- **prp:** Add frontend test framework PRD; update umu deferred issue status ([`945bfde`](https://github.com/yandy-r/crosshook/commit/945bfde7c90c03cbc29b9840a77c05c591a0d724))

- **tests:** Update documentation for testing pyramid and patterns ([#352](https://github.com/yandy-r/crosshook/issues/352)) ([`cffa633`](https://github.com/yandy-r/crosshook/commit/cffa633603717afb0054eadcdf2f8407da738b86))

- **agents:** Add strong worktree preference to agent rules ([`febfdab`](https://github.com/yandy-r/crosshook/commit/febfdab0f43604d0b4457839ccde0ca221724420))

- **prp:** Add Phase 4 library inspector plan; mark PRD phase 4 in-progress ([`574c3bd`](https://github.com/yandy-r/crosshook/commit/574c3bd236e95f9375531ad86018cd74843341f0))

- Add ROADMAP.md with prioritized next steps ([`b3ffc01`](https://github.com/yandy-r/crosshook/commit/b3ffc01ff38801a66d0c6512c69d9cc3f248d59e))

### Features

- **launch:** Enable umu-launcher by default for non-Steam launches (Phase 4) ([#266](https://github.com/yandy-r/crosshook/issues/266)) ([`2269ef6`](https://github.com/yandy-r/crosshook/commit/2269ef692b566fdf7b9135e2e34374630382f3d7))

- **flatpak:** Umu host runtime and install guidance (Phase 5) ([#267](https://github.com/yandy-r/crosshook/issues/267)) ([`1b39f80`](https://github.com/yandy-r/crosshook/commit/1b39f8063615c841bd4db5b10e50af84daa14753))

- **onboarding:** Steam Deck caveats, watchdog exe-name fallback, Flathub resolution (Phase 5b) ([#268](https://github.com/yandy-r/crosshook/issues/268)) ([`169d9bb`](https://github.com/yandy-r/crosshook/commit/169d9bbf97b3f7c369ea557e8d6a34f78069e168))

- **onboarding:** Sqlite-backed host readiness catalog ([#277](https://github.com/yandy-r/crosshook/issues/277)) ([`09bec4b`](https://github.com/yandy-r/crosshook/commit/09bec4b3d2bc158012c99d7b6ecdb5f0efda21c0))

- **onboarding:** Dedicated host tool dashboard page ([#270](https://github.com/yandy-r/crosshook/issues/270)) ([#278](https://github.com/yandy-r/crosshook/issues/278)) ([`c8330b7`](https://github.com/yandy-r/crosshook/commit/c8330b7f8ab88ae4ba3a30d8664f3ffd8c11a6cc))

- **flatpak:** Add GameMode + Background portal integrations ([#280](https://github.com/yandy-r/crosshook/issues/280)) ([`5c8907e`](https://github.com/yandy-r/crosshook/commit/5c8907e2c042aecc5c37ea2e956f250fead08aff))

- **protonup:** Native Proton download manager with AppImage/Flatpak parity ([#281](https://github.com/yandy-r/crosshook/issues/281)) ([`4d532d3`](https://github.com/yandy-r/crosshook/commit/4d532d3628fb586a9acafba7a310176ae7c6e8f1))

- **scripts:** Add --staged/--unstaged scope flags to lint and format ([`37f82b8`](https://github.com/yandy-r/crosshook/commit/37f82b812457f5359626c57ff7bb2ef48aa32294))

- **ui:** Add library list view mode functionality ([#349](https://github.com/yandy-r/crosshook/issues/349)) ([`caf96c8`](https://github.com/yandy-r/crosshook/commit/caf96c8f2152f6930dfb5b985c6bbd2406e3599a))

- **ux:** Add accessibility improvements for UI with ARIA labels and high contrast ([#348](https://github.com/yandy-r/crosshook/issues/348)) ([`97425ec`](https://github.com/yandy-r/crosshook/commit/97425ec1c450b1a84fa0093afa2565f2176c615b))

- **launch:** Trainer watchdog cleanup parity with game launches ([#396](https://github.com/yandy-r/crosshook/issues/396)) ([`49f0851`](https://github.com/yandy-r/crosshook/commit/49f0851f3c92d2866643f43dc5ced4ac07edf34a))

- **ui:** Add useBreakpoint and extract AppShell ([#453](https://github.com/yandy-r/crosshook/issues/453)) ([`5cd8af4`](https://github.com/yandy-r/crosshook/commit/5cd8af46a26c79de24f53556cdc7a9157a427552))

- **styles:** Adopt steel-blue palette and add legacy-palette sentinel ([#454](https://github.com/yandy-r/crosshook/issues/454)) ([`3cac2ba`](https://github.com/yandy-r/crosshook/commit/3cac2bac6a40b4a42a0a9d80e32f93ad823cb835))

- **ui:** Sidebar variants + formalized Collections section ([#455](https://github.com/yandy-r/crosshook/issues/455)) ([`388266b`](https://github.com/yandy-r/crosshook/commit/388266b5bc73ba582cfa40c2d1f87c84a55a4cca))

- **native:** Add persistent library inspector rail ([#457](https://github.com/yandy-r/crosshook/issues/457)) ([`c42287c`](https://github.com/yandy-r/crosshook/commit/c42287c9ac1e224792b6990cc32374f1fd3ecc9c))

- **ui:** Add in-shell hero detail mode ([#458](https://github.com/yandy-r/crosshook/issues/458)) ([`a9b42d6`](https://github.com/yandy-r/crosshook/commit/a9b42d6fdf3b01171e8abe87e896d02d39cec3fc))

- **ui:** Add global command palette ([#459](https://github.com/yandy-r/crosshook/issues/459)) ([`3b9d504`](https://github.com/yandy-r/crosshook/commit/3b9d5042732478e8731a16160bc4f2a5a60b5e53))

- **library:** Add ultrawide context rail pane ([#460](https://github.com/yandy-r/crosshook/issues/460)) ([`75880df`](https://github.com/yandy-r/crosshook/commit/75880df97b06bfb78e9f1d8f7103bb6b9a796f36))

- **layout:** Add responsive console status bar ([#461](https://github.com/yandy-r/crosshook/issues/461)) ([`e3325e9`](https://github.com/yandy-r/crosshook/commit/e3325e91ca252cf08cac221182cf7654b8afca87))

- **ui:** Rework dashboard routes ([#462](https://github.com/yandy-r/crosshook/issues/462)) ([`5f4533a`](https://github.com/yandy-r/crosshook/commit/5f4533a38d8a7f72015ed9ab353f2bb55f74c178))

- **ui:** Rework Install, Settings, Community, Discover routes (Phase 10) ([#463](https://github.com/yandy-r/crosshook/issues/463)) ([`1315799`](https://github.com/yandy-r/crosshook/commit/1315799664be5d0e5f5b83851a409f518be078a3))

- **ui:** Add per-route stylesheet scaffolding (Phase 10) ([`d7cdf5f`](https://github.com/yandy-r/crosshook/commit/d7cdf5f05b8dd937bf31a96d1f93f599f5e9e179))

- **ui:** Reskin Install route with dashboard panel chrome ([`dec466d`](https://github.com/yandy-r/crosshook/commit/dec466ddbac26a9c5eb8f4539d52496a569ab201))

- **ui:** Reskin Settings route with dashboard panel chrome ([`98db06a`](https://github.com/yandy-r/crosshook/commit/98db06af4dd8e129e2d7f0049e320a08b1963170))

- **ui:** Reskin Community route with dashboard panel chrome ([`d5019c0`](https://github.com/yandy-r/crosshook/commit/d5019c0062cd2aeb9ffdc346b83633cb50cd4dd5))

- **ui:** Reskin Discover route with dashboard panel chrome ([`5b8e9b5`](https://github.com/yandy-r/crosshook/commit/5b8e9b5166aecc8794b8633032de0371b0420792))

- **ui:** Reskin OnboardingWizard modal chrome ([`cf3db77`](https://github.com/yandy-r/crosshook/commit/cf3db7790b018affc6a5b0da3a9323d7ea794b27))

- **ui:** Rework Profiles and Launch routes ([#464](https://github.com/yandy-r/crosshook/issues/464)) ([`2fb57ea`](https://github.com/yandy-r/crosshook/commit/2fb57ea37c52a6816a4a97039b5ce7f1d0584d9d))

- **ui:** Phase 13 polish, accessibility, and design-token docs ([#465](https://github.com/yandy-r/crosshook/issues/465)) ([`5eaf89c`](https://github.com/yandy-r/crosshook/commit/5eaf89c1c67ed61dd4251df3c5c15997e12ee853))

- **ui:** Extend Hero Detail panel contract (phase 1) ([#480](https://github.com/yandy-r/crosshook/issues/480)) ([`0884144`](https://github.com/yandy-r/crosshook/commit/0884144ff3fbdee97e347ae677588f7ce57de3e7))

- **library:** Add sidebar running filters ([#481](https://github.com/yandy-r/crosshook/issues/481)) ([`ebfea89`](https://github.com/yandy-r/crosshook/commit/ebfea8942a3bfda669a2556726936b8a1cb98ff0))

- Pre/post launch hook schema and library breadcrumb navigation ([#483](https://github.com/yandy-r/crosshook/issues/483)) ([`cd3f841`](https://github.com/yandy-r/crosshook/commit/cd3f84121d71703e305b64c69d7dd1e02ba491b5))

- **library:** Add hero profile editor ([#485](https://github.com/yandy-r/crosshook/issues/485)) ([`b78bc6d`](https://github.com/yandy-r/crosshook/commit/b78bc6d1506112a8b7dad1c56f3c88095fc09bfa))

- **dev:** Auto-select native Tauri dev port for parallel browser dev ([`4728ab7`](https://github.com/yandy-r/crosshook/commit/4728ab7f5689fc9f41cbcd2a9ddee7ad537325c4))

- **library:** Add hero detail launch tab ([#488](https://github.com/yandy-r/crosshook/issues/488)) ([`9ed7292`](https://github.com/yandy-r/crosshook/commit/9ed72924709d011af66ca0227123cbfe9a458e6e))

- **ui:** Hero detail launch/profile parity before route removal ([#489](https://github.com/yandy-r/crosshook/issues/489)) ([`8a64ffc`](https://github.com/yandy-r/crosshook/commit/8a64ffc8f1ac4a67fd0bf22d0bd2aef595fb4a18))

- **library:** Hero detail create-profile wizard and creation flow ([#490](https://github.com/yandy-r/crosshook/issues/490)) ([`c4fe20e`](https://github.com/yandy-r/crosshook/commit/c4fe20e85b3e76f39c71706d97632f5dd0d6e47b))

- **ui:** Add library add-game entry point ([#492](https://github.com/yandy-r/crosshook/issues/492)) ([`04d9579`](https://github.com/yandy-r/crosshook/commit/04d9579d99fcc38de02e4e278c4341953ec99542))

- **library:** Add live launch hook editor ([#493](https://github.com/yandy-r/crosshook/issues/493)) ([`e7dc358`](https://github.com/yandy-r/crosshook/commit/e7dc3582bff859d6fd7fc8048486581779a7982b))

- **library:** Add overview panel deep links ([#494](https://github.com/yandy-r/crosshook/issues/494)) ([`f378cb9`](https://github.com/yandy-r/crosshook/commit/f378cb9173c5cf1bfefc9a90b91f76d9af954a49))

- **ui:** Remove legacy profile and launch routes ([#495](https://github.com/yandy-r/crosshook/issues/495)) ([`9f72b4f`](https://github.com/yandy-r/crosshook/commit/9f72b4f5470cf5e105f4eb7f5cfa3598783ab7d2))

- **hero-detail:** Remove legacy launch and profiles pages ([#496](https://github.com/yandy-r/crosshook/issues/496)) ([`6158532`](https://github.com/yandy-r/crosshook/commit/61585324374542b2e55a7e93717684b93db63bea))

- **ui:** Document command-preview tokens and remove orphaned launch panel assets ([#497](https://github.com/yandy-r/crosshook/issues/497)) ([`8db7c5a`](https://github.com/yandy-r/crosshook/commit/8db7c5a838eb2c35fee4940eceb94298f50b5f81))

- **ui:** Unify shell column chrome and library inspector layout ([`5cbbaae`](https://github.com/yandy-r/crosshook/commit/5cbbaaebac2a41b2ffda18f79264ac44e8b103b6))

- **launch:** Execute profile pre/post launch hooks ([#499](https://github.com/yandy-r/crosshook/issues/499)) ([`1385b2a`](https://github.com/yandy-r/crosshook/commit/1385b2aa6646164912ecef2b293848ae7eabbc4e))

### Refactoring

- **platform:** Document and enforce host-command gateway contract ([#279](https://github.com/yandy-r/crosshook/issues/279)) ([`aaac51b`](https://github.com/yandy-r/crosshook/commit/aaac51bff4bb774757d9bc37576e248d619272ed))

- **metadata:** Split mod.rs (3,747 → 136 lines) into per-domain submodules ([#292](https://github.com/yandy-r/crosshook/issues/292)) ([`ebacd78`](https://github.com/yandy-r/crosshook/commit/ebacd78ea4bfb01bb37847feb8cfdfeb5eebaa94))

- **launch:** Split script runner modules ([`c9f5363`](https://github.com/yandy-r/crosshook/commit/c9f53637f29c85a50300c4e3aaa502917b877e0c))

- **launch:** Reuse umu test helper ([`2166b2b`](https://github.com/yandy-r/crosshook/commit/2166b2bea3ebf820a5256eee2e6fb4bdd1f5a800))

- **launch:** Split request module ([#296](https://github.com/yandy-r/crosshook/issues/296)) ([`0917301`](https://github.com/yandy-r/crosshook/commit/09173016ee91379d6fa28cfb4a30a5dfdb856dd9))

- **profile:** Split useProfile into modular hooks ([#298](https://github.com/yandy-r/crosshook/issues/298)) ([`ab4bace`](https://github.com/yandy-r/crosshook/commit/ab4bace59d6d524c9718c2e662b93cd63c633bed))

- **protonup:** Split install module ([#300](https://github.com/yandy-r/crosshook/issues/300)) ([`51fbd50`](https://github.com/yandy-r/crosshook/commit/51fbd509621a8dfee32851301907f1402da66a59))

- **core:** Split launch/preview and profile/models into submodules ([#302](https://github.com/yandy-r/crosshook/issues/302)) ([`4268d30`](https://github.com/yandy-r/crosshook/commit/4268d301e9f1e42b94f31ddccde399f9ec67db89))

- **metadata:** Split migrations.rs into per-tier submodules ([#311](https://github.com/yandy-r/crosshook/issues/311)) ([`56a60a2`](https://github.com/yandy-r/crosshook/commit/56a60a2bc2e4449a86c84847f515fe0c0b0e29c1))

- **commands:** Split commands/profile.rs into per-domain submodules ([#312](https://github.com/yandy-r/crosshook/issues/312)) ([`5772148`](https://github.com/yandy-r/crosshook/commit/577214840d3330f11864460707ce88a1bc13a06b))

- **components:** Split SettingsPanel into per-section subcomponents ([#314](https://github.com/yandy-r/crosshook/issues/314)) ([`b5a3c74`](https://github.com/yandy-r/crosshook/commit/b5a3c74ec793e589fabeb79601749460788d3f2b))

- **export:** Split launcher_store into per-domain submodules ([#315](https://github.com/yandy-r/crosshook/issues/315)) ([`491310d`](https://github.com/yandy-r/crosshook/commit/491310d8568eea444f6a74e77115d9bb887b54d8))

- **platform:** Split runtime platform module ([#318](https://github.com/yandy-r/crosshook/issues/318)) ([`fdeb464`](https://github.com/yandy-r/crosshook/commit/fdeb46400663158e0e4f3ecce42b1b878b2116be))

- **export:** Split launcher export module ([#319](https://github.com/yandy-r/crosshook/issues/319)) ([`0a11925`](https://github.com/yandy-r/crosshook/commit/0a1192584c3d8a8c5bd634d47754c30b263328d0))

- **profile:** Split toml_store into directory module ([#321](https://github.com/yandy-r/crosshook/issues/321)) ([`4a0474a`](https://github.com/yandy-r/crosshook/commit/4a0474ae6ebd7ab9d9acf608f7ec1eb361041a59))

- **components:** Split HealthDashboardPage into per-section subcomponents ([#320](https://github.com/yandy-r/crosshook/issues/320)) ([`ed8de69`](https://github.com/yandy-r/crosshook/commit/ed8de694c0803916d8cb6c585711a63b708f739c))

- **profile:** Split health into directory module ([#326](https://github.com/yandy-r/crosshook/issues/326)) ([`17af29a`](https://github.com/yandy-r/crosshook/commit/17af29aae860ddcabe725d47de86d63c092853dc))

- **launch:** Split runtime_helpers into directory module ([#327](https://github.com/yandy-r/crosshook/issues/327)) ([`d1298d9`](https://github.com/yandy-r/crosshook/commit/d1298d9f27d79f69b78178af6b537242e225bb8d))

- Split launch and profiles modules ([`e8633dd`](https://github.com/yandy-r/crosshook/commit/e8633dd02870ecfef26c948f279474b0658794df))

- **steam:** Split proton.rs into directory module ([#330](https://github.com/yandy-r/crosshook/issues/330)) ([`df9b544`](https://github.com/yandy-r/crosshook/commit/df9b544e1d035b36eeb12ad5b39c9b8ec0588563))

- **components:** Split LaunchPanel into launch-panel subdirectory ([#331](https://github.com/yandy-r/crosshook/issues/331)) ([`fc2653a`](https://github.com/yandy-r/crosshook/commit/fc2653a04bfd781f9b8a3bb8a48eb409ad03ef12))

- **community:** Split taps.rs into smaller modules ([#382](https://github.com/yandy-r/crosshook/issues/382)) ([`bbea753`](https://github.com/yandy-r/crosshook/commit/bbea753b89cbfed6959125c20c54793120ffa579))

- Watchdog.rs into smaller modules ([#384](https://github.com/yandy-r/crosshook/issues/384)) ([`0986845`](https://github.com/yandy-r/crosshook/commit/0986845a445a81891fda4336943f32cb8ebe6fdc))

- Client.rs into smaller modules for better maintainability ([#385](https://github.com/yandy-r/crosshook/issues/385)) ([`706ce20`](https://github.com/yandy-r/crosshook/commit/706ce2067d0c076ed5f12dd5b7a15302492c274e))

- **cli:** Split main into modules ([#383](https://github.com/yandy-r/crosshook/issues/383)) ([`807973d`](https://github.com/yandy-r/crosshook/commit/807973dde0b1d19e809bec9b89dc5006cf6955ad))

- **metadata:** Split community_index.rs into smaller modules ([#386](https://github.com/yandy-r/crosshook/issues/386)) ([`60c2856`](https://github.com/yandy-r/crosshook/commit/60c2856a26ff22f1b0f7b9427a784056f75b8f6f))

- **settings:** Split settings module ([#387](https://github.com/yandy-r/crosshook/issues/387)) ([`22c49ba`](https://github.com/yandy-r/crosshook/commit/22c49baa896242cd8c15daddf379f0b375639efe))

- **core:** Split capability.rs into smaller modules ([#388](https://github.com/yandy-r/crosshook/issues/388)) ([`92ea71d`](https://github.com/yandy-r/crosshook/commit/92ea71d44d234848bf340d88e76397670d8cbc37))

- **profile:** Split migration module ([#389](https://github.com/yandy-r/crosshook/issues/389)) ([`004929c`](https://github.com/yandy-r/crosshook/commit/004929c4986244393080fd0067e6d99d8d7e7c59))

- Split readiness.rs into smaller modules ([#390](https://github.com/yandy-r/crosshook/issues/390)) ([`595952b`](https://github.com/yandy-r/crosshook/commit/595952bd9bfa1ecf95c7658fad105429f557757d))

- Split diagnostics.rs into smaller modules ([#392](https://github.com/yandy-r/crosshook/issues/392)) ([`8db1888`](https://github.com/yandy-r/crosshook/commit/8db1888e7eab10125b04066b79e84b02742eac4d))

- Split client.rs into smaller modules ([#393](https://github.com/yandy-r/crosshook/issues/393)) ([`d75ed72`](https://github.com/yandy-r/crosshook/commit/d75ed72162913bd3ba9e9d87d16e76f605f81de9))

- Split health.rs into smaller modules ([#391](https://github.com/yandy-r/crosshook/issues/391)) ([`84f7df9`](https://github.com/yandy-r/crosshook/commit/84f7df9107502b38811ec571736f1d5c6affde78))

- Config_history_store.rs into smaller modules ([#395](https://github.com/yandy-r/crosshook/issues/395)) ([`a2cb8f1`](https://github.com/yandy-r/crosshook/commit/a2cb8f1e22951f1dfce9c3de0ab6967f9b968b52))

- LaunchOptimizationsPanel into smaller modules ([#397](https://github.com/yandy-r/crosshook/issues/397)) ([`940fdd6`](https://github.com/yandy-r/crosshook/commit/940fdd65f5635399c3985d0bac8e35d0bae424b2))

- UseGamepadNav.ts into smaller modules ([#398](https://github.com/yandy-r/crosshook/issues/398)) ([`441de35`](https://github.com/yandy-r/crosshook/commit/441de3577ef409a0cfd8264dafa3ec2f50fb5e4c))

- CommunityImportWizardModal into smaller modules ([#399](https://github.com/yandy-r/crosshook/issues/399)) ([`909b1e2`](https://github.com/yandy-r/crosshook/commit/909b1e2de957741f889184849e2091272f530181))

- Service.rs into smaller modules ([#400](https://github.com/yandy-r/crosshook/issues/400)) ([`6c81399`](https://github.com/yandy-r/crosshook/commit/6c81399504e44057925ba9973f8cf645a1895110))

- Profile.ts into smaller modules ([#401](https://github.com/yandy-r/crosshook/issues/401)) ([`942bfc5`](https://github.com/yandy-r/crosshook/commit/942bfc5961b3914fe15b74cf04c416c1cd740f45))

- Exchange.rs into smaller modules ([#402](https://github.com/yandy-r/crosshook/issues/402)) ([`9e202c3`](https://github.com/yandy-r/crosshook/commit/9e202c38b020aefaf9968399bddc447da694d170))

- Optimizations.rs into smaller modules ([#403](https://github.com/yandy-r/crosshook/issues/403)) ([`0deda41`](https://github.com/yandy-r/crosshook/commit/0deda41c90495e27b122de67c93bd974c3955896))

- Split ConfigHistoryPanel into smaller modules ([#404](https://github.com/yandy-r/crosshook/issues/404)) ([`d51975c`](https://github.com/yandy-r/crosshook/commit/d51975ccd818280148898323c05d5dd2c9f56dba))

- Onboarding.ts into smaller modules ([#405](https://github.com/yandy-r/crosshook/issues/405)) ([`cfd6b54`](https://github.com/yandy-r/crosshook/commit/cfd6b54f22bc52da6a775ef7afccd2e4635a203a))

- Catalog.rs into smaller modules ([#406](https://github.com/yandy-r/crosshook/issues/406)) ([`63b82b8`](https://github.com/yandy-r/crosshook/commit/63b82b8e9f979a4771b60da27c0c37459d7ba730))

- Prefix_health.rs into smaller modules ([#408](https://github.com/yandy-r/crosshook/issues/408)) ([`4788611`](https://github.com/yandy-r/crosshook/commit/4788611d2adf36a77f5dab3b9d5bdc7065479b5b))

- MigrationReviewModal into smaller modules ([#409](https://github.com/yandy-r/crosshook/issues/409)) ([`5a728fb`](https://github.com/yandy-r/crosshook/commit/5a728fb8cf6e6dce002041f9b40041ccc3241f1a))

- Collection_exchange.rs into smaller modules ([#410](https://github.com/yandy-r/crosshook/issues/410)) ([`360c45f`](https://github.com/yandy-r/crosshook/commit/360c45f8fc11d5d9c0f3c04a3fc53bd9a0073036))

- **ui:** Split OnboardingWizard into stage bodies ([`b1ba2a3`](https://github.com/yandy-r/crosshook/commit/b1ba2a3622e1f0474465aa99107176a6ed5538fa))

- **ui:** Split CommunityBrowser into sibling sections ([`a28a20c`](https://github.com/yandy-r/crosshook/commit/a28a20c1d6780c2d3388fd7d3b411b8dd9658746))

### Tests

- **frontend:** Add vitest and rtl seed suite ([#350](https://github.com/yandy-r/crosshook/issues/350)) ([`e3f1f97`](https://github.com/yandy-r/crosshook/commit/e3f1f97caf2367a01feda391e40b02ba95ef2c9f))

- **coverage:** Add tests to achieve 60% line coverage on critical surfaces ([#353](https://github.com/yandy-r/crosshook/issues/353)) ([`046b131`](https://github.com/yandy-r/crosshook/commit/046b131db6ef4d234dbbe4730a73e5a0fd3166ee))

- **e2e:** Spike Tauri E2E via tauri-driver for WebKitGTK coverage gap ([#354](https://github.com/yandy-r/crosshook/issues/354)) ([`d840700`](https://github.com/yandy-r/crosshook/commit/d84070043374f65553dc545b5da197abbb250a44))

- **integration:** Integrate Vitest and Playwright as CI jobs ([#351](https://github.com/yandy-r/crosshook/issues/351)) ([`01fbe9f`](https://github.com/yandy-r/crosshook/commit/01fbe9f81ea465cc22660ba2383d8084ff8fa153))

- Evaluate ts-rs export of Rust arg/return shapes to TypeScript ([#355](https://github.com/yandy-r/crosshook/issues/355)) ([`eaf6ed1`](https://github.com/yandy-r/crosshook/commit/eaf6ed118a86b735e51016524e6e5b50abc0396e))

- **ui:** Add RTL shell coverage for Phase 10 routes ([`22f6526`](https://github.com/yandy-r/crosshook/commit/22f65264fd57dca9f96a2f04a435f63cdbd13bdc))

- **smoke:** Lock in Phase 10 chrome assertions ([`93da9a9`](https://github.com/yandy-r/crosshook/commit/93da9a9132e1631a7b0ade3952bdea0b175d1356))

- **ui:** Phase 12 responsive smoke + sweep expansion ([#424](https://github.com/yandy-r/crosshook/issues/424)) ([`56b695c`](https://github.com/yandy-r/crosshook/commit/56b695caa3d9c021c6f870f6a259ed896bb11fa8))

## [v0.2.11] - 2026-04-15

### Bug Fixes

- **launch:** Suppress gamescope spam ([#226](https://github.com/yandy-r/crosshook/issues/226)) ([`683dd83`](https://github.com/yandy-r/crosshook/commit/683dd833782d55d57c34c35473f38bbb6235330e))

- **launch:** Clean up stale gamescope processes after proton_run exits ([#228](https://github.com/yandy-r/crosshook/issues/228)) ([`9dacd5d`](https://github.com/yandy-r/crosshook/commit/9dacd5d2a0de0a771f541c99b94ef419c2091965))

- **launch:** Flatpak gamescope PID capture and watchdog cleanup ([`a23850f`](https://github.com/yandy-r/crosshook/commit/a23850fa3780db3ed26230626ada6f33dbdbbf60))

- **ui:** A11y pass and biome lint cleanup ([#232](https://github.com/yandy-r/crosshook/issues/232)) ([`63ec452`](https://github.com/yandy-r/crosshook/commit/63ec452824dd742b1af47c422d27c2086e7e8c98))

### Documentation

- **prps:** Add auto-trainer gamescope plan and spec ([`69dca48`](https://github.com/yandy-r/crosshook/commit/69dca480e1ebe8cffcd81dc54f9b4b4e3aeae203))

- **prd:** Add GitHub issue tables to umu-launcher migration PRD ([`92ac832`](https://github.com/yandy-r/crosshook/commit/92ac832052334fd2a5e23137d8906e91317a60b5))

### Features

- Auto-derive trainer gamescope config ([#231](https://github.com/yandy-r/crosshook/issues/231)) ([`70a9ddc`](https://github.com/yandy-r/crosshook/commit/70a9ddc1c2d5925355245c90c2b41756225fa00a))

- **launch:** Set PROTON_VERB per builder for Proton direct path (Phase 1) ([#260](https://github.com/yandy-r/crosshook/issues/260)) ([`7c3b538`](https://github.com/yandy-r/crosshook/commit/7c3b538e88b120e0db79566ec8aa68d3cf0be6a0))

- Add phase 2 sandbox allowlist plumbing ([#261](https://github.com/yandy-r/crosshook/issues/261)) ([`f154689`](https://github.com/yandy-r/crosshook/commit/f1546898c33952f2553ba963e196542507d25b5a))

- **scripts:** Add --modified option to format and lint scripts ([`195dc39`](https://github.com/yandy-r/crosshook/commit/195dc39d834cc8e24b85e37771128c923d99b091))

- **launch:** Add umu opt-in for non-Steam launches (Phase 3) ([#264](https://github.com/yandy-r/crosshook/issues/264)) ([`d8a851c`](https://github.com/yandy-r/crosshook/commit/d8a851c4e282485b9e6af5a1710944dcee6c5817))

## [v0.2.10-flatpak] - 2026-04-13

### Bug Fixes

- **ci:** Build flatpak from staged native assets ([`c0c6d08`](https://github.com/yandy-r/crosshook/commit/c0c6d08e07409845e90f4dc5704724c4d5e53efc))

- **build:** Restore lint typecheck compatibility ([`189895d`](https://github.com/yandy-r/crosshook/commit/189895d14e48b689d420fbd782f56247e5f57b13))

- **build:** Address biome lint regressions ([`068b844`](https://github.com/yandy-r/crosshook/commit/068b84454ff0636f3562ad4f4f0cfb13d3b67045))

- **launch:** Align trainer network isolation behavior ([#223](https://github.com/yandy-r/crosshook/issues/223)) ([`56fe553`](https://github.com/yandy-r/crosshook/commit/56fe55319d37bd1762a80845504af7e44b94eb49))

- **launch:** Align Steam trainer with proton_run and Flatpak parity ([#227](https://github.com/yandy-r/crosshook/issues/227)) ([`7fd52b2`](https://github.com/yandy-r/crosshook/commit/7fd52b2b8708ecc392864a18a634e20c84c71a50))

### Build

- **icons:** Automate AppImage icon sync pipeline ([`bf56733`](https://github.com/yandy-r/crosshook/commit/bf567336f00afce8eb0ee3acf2903a3d387449cd))

- **lint:** Add code quality tooling standard ([`cada9bb`](https://github.com/yandy-r/crosshook/commit/cada9bbde1c575d03efbc7753920cccddc36843a))

### Documentation

- Add issue asset and task log for trainer gamescope tracking ([`5e33353`](https://github.com/yandy-r/crosshook/commit/5e33353c939f541320563517844de4deccd8cdc6))

### Features

- **ui:** Add launch pipeline stepper phase 1 ([#191](https://github.com/yandy-r/crosshook/issues/191)) ([`5a3855e`](https://github.com/yandy-r/crosshook/commit/5a3855e5506245c8948a319774431bae1db89d87))

- **ui:** Add preview-derived launch pipeline status ([#192](https://github.com/yandy-r/crosshook/issues/192)) ([`5c01cd3`](https://github.com/yandy-r/crosshook/commit/5c01cd346f942e22dd3b54dd9ef706863fe0d800))

- **ui:** Add live launch pipeline phase 3 overlay ([#193](https://github.com/yandy-r/crosshook/issues/193)) ([`ed8dacd`](https://github.com/yandy-r/crosshook/commit/ed8dacd0dfe71bad998122dfd1063f6ae65f255c))

- **ui:** Add launch pipeline phase 4 polish & accessibility ([#194](https://github.com/yandy-r/crosshook/issues/194)) ([`7f4b618`](https://github.com/yandy-r/crosshook/commit/7f4b61824c57c2b4825b598c1513324819a43cd8))

- **build:** Move native outputs to xdg paths ([`480d151`](https://github.com/yandy-r/crosshook/commit/480d1516e045abae4f415da42cb45525b9d61e16))

- **flatpak:** Harden phase 3 process execution ([#214](https://github.com/yandy-r/crosshook/issues/214)) ([`f8eada9`](https://github.com/yandy-r/crosshook/commit/f8eada95e7a843e1010f91cedad78e2b10313ee0))

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
