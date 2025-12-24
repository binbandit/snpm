# Changelog

All notable changes to this project will be documented in this file.

## [2025.12.24] - 2025-12-24

### <!-- 1 -->🐛 Bug Fixes

- *(release)* Correct changelog generation and skip empty daily releases ([f3abd90](https://github.com/binbandit/snpm/commit/f3abd90d4278c5bb603e80c77bc72e26abc24e79))
- *(release)* Fix changelog generation and skip daily releases on ci commits ([3e46b3b](https://github.com/binbandit/snpm/commit/3e46b3bfbcba8bb4c269f07d0b0bdb93229e864e))

### <!-- 2 -->♻️ Refactor

- *(cli)* Restructure into modular commands ([f30b6d5](https://github.com/binbandit/snpm/commit/f30b6d57b16dae1eae57987eba97f8d3c86e96ff))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- *(release)* Bump version to 2025.12.23 [skip ci] ([a00437f](https://github.com/binbandit/snpm/commit/a00437fac36d03e69d45b453bbdad30f53e32cfd))

## [2025.12.23] - 2025-12-22

### <!-- 7 -->⚙️ Miscellaneous Tasks

- *(release)* Bump version to 2025.12.22 [skip ci] ([e7c69c1](https://github.com/binbandit/snpm/commit/e7c69c13ed2ce222bd459754f84a1fcaad60e3c6))

## [2025.12.22] - 2025-12-21

### <!-- 7 -->⚙️ Miscellaneous Tasks

- *(release)* Bump version to 2025.12.21 [skip ci] ([8c947ff](https://github.com/binbandit/snpm/commit/8c947ffa201a8a601ce881863f3224ff8b28b2d6))

## [2025.12.21] - 2025-12-20

### <!-- 7 -->⚙️ Miscellaneous Tasks

- *(release)* Bump version to 2025.12.20 [skip ci] ([356d375](https://github.com/binbandit/snpm/commit/356d375785115c388e4d57b2372c2b62a0f23642))

## [2025.12.20] - 2025-12-19

### <!-- 7 -->⚙️ Miscellaneous Tasks

- *(release)* Bump version to 2025.12.19 [skip ci] ([d4520f3](https://github.com/binbandit/snpm/commit/d4520f3e94ae2f6f977e9f702958d2fe97c19176))

## [2025.12.19] - 2025-12-19

### <!-- 7 -->⚙️ Miscellaneous Tasks

- *(ci)* Remove backfill changelog workflow ([d443c16](https://github.com/binbandit/snpm/commit/d443c16902a8a6ba8f03fb584af22778faf293d8))

## [2025.12.17] - 2025-12-16

### <!-- 2 -->♻️ Refactor

- *(cli)* Pass version into console header ([08821ab](https://github.com/binbandit/snpm/commit/08821abfe50f8b5925356cc5805d9e56e59e93cf))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- *(release)* Use git-cliff for changelog generation ([32e4775](https://github.com/binbandit/snpm/commit/32e47753e67e4131f72936b0472b3690df26823e))
- *(releases)* Add workflow to backfill changelogs ([74f5838](https://github.com/binbandit/snpm/commit/74f583874cf613f7c4827f3ab0899190831c33d9))
- *(ci)* Disable dry-run and trim release body ([65df485](https://github.com/binbandit/snpm/commit/65df485ddfb7142f3c17ddcead25e322f4ad28f8))
- *(backfill-changelogs)* Add logging and fail on cliff errors ([58cf324](https://github.com/binbandit/snpm/commit/58cf324397d390c699ab89e2681cc4c5a8608c31))

## [2025.12.16] - 2025-12-16

### <!-- 0 -->🚀 Features

- *(cli)* Add dlx command and spx npm shim ([5dab200](https://github.com/binbandit/snpm/commit/5dab200e7ffcf51966d051485375df1022a62185))

### <!-- 3 -->📚 Documentation

- *(cli)* Document snpm dlx command and spx alias ([945cc9f](https://github.com/binbandit/snpm/commit/945cc9f6241c8396a42946e64939b188d8b3f506))
- *(roadmap)* Add roadmap page and nav entry ([6165b4a](https://github.com/binbandit/snpm/commit/6165b4a0c96d0218eed36f54cf8a325b44f235f0))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- *(ci)* Allow npm publish job to continue ([58ca1c7](https://github.com/binbandit/snpm/commit/58ca1c701754aea8db70ad5c7d9f56ee428c208b))

## [2025.12.15] - 2025-12-14

### <!-- 0 -->🚀 Features

- *(error)* [**breaking**] Expand SnpmError for precise IO/HTTP/semver errors ([a5121fe](https://github.com/binbandit/snpm/commit/a5121febd2c1d7988979bb77bad445a77092d521))
- *(config)* Add packages_dir accessor ([2f3a094](https://github.com/binbandit/snpm/commit/2f3a094bac241996eaaa8761a0fabd3fbcd3279b))
- *(project)* Write manifest to disk ([607c5f6](https://github.com/binbandit/snpm/commit/607c5f6a149e6a3ff738e43b4864bedce8d85518))
- *(install)* Reuse lockfile to skip resolution ([2ac57af](https://github.com/binbandit/snpm/commit/2ac57afe6d9f2428ab8b83f2e0977258759e5a66))
- *(scripts)* Add snpm run, link bins for scripts ([688c8ce](https://github.com/binbandit/snpm/commit/688c8ce132ae1d006dc480eddf9c4452e3b8e0ff))
- *(cli)* Add remove command to uninstall deps ([4b24582](https://github.com/binbandit/snpm/commit/4b24582ad8fa462c011e978a46294fbed8506d96))
- *(linker)* Use symlinks with copy fallback ([c9ded66](https://github.com/binbandit/snpm/commit/c9ded667aac4124a4f9f51fc6cad2c2b9a8b38e0))
- *(cli)* Add add cmd; install supports dev/prod ([20d0eee](https://github.com/binbandit/snpm/commit/20d0eee25b9af8648e36bf2622564b8f309cff29))
- *(workspace)* Enable YAML monorepo installs ([75a64a3](https://github.com/binbandit/snpm/commit/75a64a3cc04effd54af0b778f502666c0fd8f982))
- *(install)* Add --frozen-lockfile support ([0615170](https://github.com/binbandit/snpm/commit/0615170427b114b82bc8a16a841ac4e0dc5a2715))
- *(registry)* Allow cloning of RegistryPackage ([3749d5c](https://github.com/binbandit/snpm/commit/3749d5c38873192eb5773cf114199b4bba4a5ec4))
- Add workspace catalog resolution and local linking ([94644fe](https://github.com/binbandit/snpm/commit/94644fe08c90bf4ee511be94bee256e8633ec62c))
- *(install)* Aggregate workspace root deps ([48cb1b2](https://github.com/binbandit/snpm/commit/48cb1b238e1c5ad08690291f9a49f3d37c8fa382))
- *(cli)* Support add to specific workspace ([247e5c0](https://github.com/binbandit/snpm/commit/247e5c0de834586e0c74bc15621c7a5fd005a134))
- *(resolve)* Add os/cpu and optional deps support ([a95e39d](https://github.com/binbandit/snpm/commit/a95e39d421c0d9f67452286b8dba74fc657ac0f9))
- *(lifecycle)* Add controlled dep install scripts ([069ac33](https://github.com/binbandit/snpm/commit/069ac33c574cef9cff1b887020db8bf91f6ac434))
- *(workspace)* Load catalogs from root file ([ecb92f0](https://github.com/binbandit/snpm/commit/ecb92f0f03685309a3f95b37c176ec7c6995b131))
- *(install)* Add force flag and package age guard ([76cf37d](https://github.com/binbandit/snpm/commit/76cf37dc5f86875ff74883e2ba5324dc6a9ea4b5))
- *(linker)* Symlink non-build deps to store ([1dcaf91](https://github.com/binbandit/snpm/commit/1dcaf91ba243e226a4ded6300751c3305d8de28b))
- *(cli)* Add init, upgrade and outdated cmds ([ef9a61c](https://github.com/binbandit/snpm/commit/ef9a61ce9b708d1704cadac30c667f09b3da19a2))
- *(install)* Support registry and protocol overrides ([da3857d](https://github.com/binbandit/snpm/commit/da3857ddcfc860c6bba09fa5b1c4a408457af48c))
- *(config)* Honor global rc files for registries ([0031d67](https://github.com/binbandit/snpm/commit/0031d67316397be9ea06931e4677e6cb32311880))
- *(install)* Add snpm manifest overrides support ([fa7bafd](https://github.com/binbandit/snpm/commit/fa7bafdddea62a94b3998294771a540b0da5919e))
- *(registry)* Support custom npm-like registries ([e506843](https://github.com/binbandit/snpm/commit/e5068438c85eaf4d436e313aee5d5395fbf56b6f))
- *(config)* Add auth token support for registries ([4f958fe](https://github.com/binbandit/snpm/commit/4f958fe9a48dfae0650c9176833ac381b813ac48))
- *(resolve)* Enforce non-optional peer deps ([d103cbc](https://github.com/binbandit/snpm/commit/d103cbc05864260dbf6b4585557846d8550b803a))
- *(install)* Respect npm/jsr protocols from manifest ([2b77112](https://github.com/binbandit/snpm/commit/2b77112af69c7f17cf7b6da92c16601297bc7681))
- *(cli)* Add styled console output helper ([141b642](https://github.com/binbandit/snpm/commit/141b642af2fce9672722be4d9dcc1b8528a299d9))
- *(install)* Skip existing deps for idempotent add ([ae8b0b9](https://github.com/binbandit/snpm/commit/ae8b0b9461dc9a41b1a7e057df0a4d296446477a))
- *(config)* Honor SNPM_HOME for cache and data dirs ([4e2f123](https://github.com/binbandit/snpm/commit/4e2f1234c126288de3ab8d4a61debaefba2583dd))
- *(resolve)* Add concurrent resolution and semver lib ([3298ce6](https://github.com/binbandit/snpm/commit/3298ce6da898dac00ea7d1267e360626bf6fce77))
- *(semver)* Align range parsing with node semantics ([efaa541](https://github.com/binbandit/snpm/commit/efaa5416a11fa65f1d4fd750e61ecb912f911bee))
- *(install)* Add async store prefetching during resolve ([eb5851c](https://github.com/binbandit/snpm/commit/eb5851ccec92ac66bc9e3a0a2eb76f853bad64cb))
- *(console)* Show install summary with timing ([911e127](https://github.com/binbandit/snpm/commit/911e127d06f0a3f6bf154855c454708210b885c7))
- *(upgrade)* Support targeted package updates ([7a34b85](https://github.com/binbandit/snpm/commit/7a34b851822f951ed91f8d6c09db7c5f15d5ea14))
- *(config)* Support env and host-normalized tokens ([f311238](https://github.com/binbandit/snpm/commit/f31123883eb61f251ec8721642266293910dee92))
- *(cli)* Support install targeting workspace ([5c4c558](https://github.com/binbandit/snpm/commit/5c4c55843a2a124ff54ef034ba14fb6ef039fd13))
- *(run)* Add recursive workspace script runner ([bf764e3](https://github.com/binbandit/snpm/commit/bf764e329180a0db50a6bdeb0d510dae7b780a2d))
- *(auth)* Add login command to store tokens ([da34812](https://github.com/binbandit/snpm/commit/da34812bab91c811e2fe097eb57cec2c4cfe5d94))
- *(install)* Support catalog usage without workspace ([7cd784b](https://github.com/binbandit/snpm/commit/7cd784b9b2ed6cf0919edecc1a443678e9a947dd))
- *(auth)* Add scoped login/logout and web flow ([0b2c8a2](https://github.com/binbandit/snpm/commit/0b2c8a268075e767d862dae95bc6cb215d01019f))
- *(console)* Add structured CLI output headers ([ed8bd4d](https://github.com/binbandit/snpm/commit/ed8bd4dd0d5f211f47913370c39b8906209ffdfe))
- Add daily release pipeline for snpm ([5f59c69](https://github.com/binbandit/snpm/commit/5f59c69851b367085a5ce2995bb3057e11d4112f))
- Add npm package and dual MIT/Apache-2.0 license ([52d9530](https://github.com/binbandit/snpm/commit/52d9530db15f2ffc30055cc793d7f2e2fd7d4549))
- *(cli)* Show version via clap metadata ([ca534f1](https://github.com/binbandit/snpm/commit/ca534f100412f57656f14e1a4155f77ea193efda))
- *(docs-site)* Add Next.js docs app with CI deploy ([c51015f](https://github.com/binbandit/snpm/commit/c51015f4c1ded646b9dc8dd4028295cdd141bf32))
- *(api)* Disable revalidation for search route ([f62817e](https://github.com/binbandit/snpm/commit/f62817e8f8923d5f99b2424c0128f875b1bd8118))
- *(home-layout)* Add homepage SEO metadata ([d4613ab](https://github.com/binbandit/snpm/commit/d4613ab44a1797fc7092ebb33b9ffb205bef9691))
- *(linker)* Add configurable hoisting modes ([992bddf](https://github.com/binbandit/snpm/commit/992bddfcad187567b6b5e9c3ccb0f52ca1152bd6))
- *(resolve)* Add configurable strict peer checks ([761a516](https://github.com/binbandit/snpm/commit/761a516bbeb8cf5d572ee7ba367b103498b92271))
- *(cli)* Add configurable verbose logging ([15c1c7c](https://github.com/binbandit/snpm/commit/15c1c7c3fb5d830cf8ff745a5cf57a0976699f7e))
- *(linker)* Add configurable link backend and reuse deps ([7ed6c2a](https://github.com/binbandit/snpm/commit/7ed6c2a951c01cf45ce56b9c65e741c2483352d3))
- *(registry)* Add cached metadata with TTL config ([5bbe0b3](https://github.com/binbandit/snpm/commit/5bbe0b31968ab69fddbbf5ec85ba6694eecf5dcf))
- *(install)* Add lockfile-based integrity cache ([e8ff3c7](https://github.com/binbandit/snpm/commit/e8ff3c76fdb8c08008a3f1f158ae2a5b5261125f))
- *(workspace)* Support npm-style workspaces ([8f7ec60](https://github.com/binbandit/snpm/commit/8f7ec60eda931c76634df6a6ff233a77edaa46b6))
- *(install)* Speed up workspace installs ([e60163e](https://github.com/binbandit/snpm/commit/e60163ee87218f88a6cc9fd2e0dccb162d297ee9))
- *(core)* Add AuthScheme and enhance config loading/validation ([acc61a2](https://github.com/binbandit/snpm/commit/acc61a21f6ca30f71aa5ef95a9ac9366d5cadcc3))
- *(linker)* Link bundled dependency bins ([ad5f20c](https://github.com/binbandit/snpm/commit/ad5f20ce15be57724f4ea8afdfdb8fdd5f2af14d))
- *(registry)* Support file and git sources ([bda4066](https://github.com/binbandit/snpm/commit/bda4066252b09c64e2fab369347e638a476fcece))
- *(justfile)* Add cargo check helper recipe ([3f06ad4](https://github.com/binbandit/snpm/commit/3f06ad4a421e89613e8eb380ae982d4cb5204b88))
- *(protocols)* Add file, git, and jsr resolvers ([64b40b7](https://github.com/binbandit/snpm/commit/64b40b7bb904e8b2f92ea379f9adafba9a2f3d84))
- *(cache)* Add registry metadata caching ([4afa047](https://github.com/binbandit/snpm/commit/4afa047cc526e41c5961ae45d9556f2f3d888d44))
- *(lifecycle)* Run full script lifecycle on install ([3d6964c](https://github.com/binbandit/snpm/commit/3d6964c54aa9d52ee5313fb9eee044d68a8e4766))

### <!-- 1 -->🐛 Bug Fixes

- Handle `||` version range ([496a8a1](https://github.com/binbandit/snpm/commit/496a8a16381a99444f2a06ad2d5f2ef15211a115))
- Fixed bin link generation ([ec5845e](https://github.com/binbandit/snpm/commit/ec5845e50b2a09483c2ce1f7a330fecf5eab09ea))
- *(linker)* Sanitize bin names, avoid nested paths ([e78786b](https://github.com/binbandit/snpm/commit/e78786bfd7fc4e0ad13dccbd1c17d4c151eb8b41))
- *(install)* Use dev flag to match options rename ([af6cdd7](https://github.com/binbandit/snpm/commit/af6cdd7224513d510b083ba78fcdf1eaf4566d20))
- *(install)* Use lockfile only with dev installs ([612454b](https://github.com/binbandit/snpm/commit/612454bea6d196ba8c9adf53a0741d7361cbc05a))
- *(linker)* Only skip pure dev deps when linking ([f871e0d](https://github.com/binbandit/snpm/commit/f871e0d5cdb69fd74b870477733135d5ad01b66f))
- *(resolve)* Preinsert package to avoid cycles ([44c87dc](https://github.com/binbandit/snpm/commit/44c87dc47c576a2307be5224bdee310209670e00))
- *(resolve)* Support protocol specs in overrides ([f3a820a](https://github.com/binbandit/snpm/commit/f3a820a6c5710546d583210a6ee3edecaf7e116b))
- *(cli)* Avoid mut index in workspace install ([c4ba2e5](https://github.com/binbandit/snpm/commit/c4ba2e5dc90066b42cc5713465d0f939dd01a613))
- *(cli)* Update workspace install loop and lockfile ([cc631ab](https://github.com/binbandit/snpm/commit/cc631aba0bc4496d62826dbfc7f6632c7e3565ab))
- *(resolve)* Normalize complex semver ranges ([10a9ba9](https://github.com/binbandit/snpm/commit/10a9ba9be606a7180c383a5f2b87543b672541d8))
- *(resolve)* Handle complex semver range parsing ([97b62aa](https://github.com/binbandit/snpm/commit/97b62aa48a6cdddb1d10d2aef9b4f2e3f2e7aa5d))
- *(registry)* Reuse client and set npm accept header ([ab9d12e](https://github.com/binbandit/snpm/commit/ab9d12edebd30a448742361781339c518986a0f6))
- *(resolve)* Normalize AND ranges with commas ([745bf88](https://github.com/binbandit/snpm/commit/745bf88369ba96822626bca8ac05e618f73c5c23))
- *(resolve)* Handle operator spacing in ranges ([6c85dc7](https://github.com/binbandit/snpm/commit/6c85dc725de79a20f62342c09d19ae2859ca2710))
- *(semver)* Simplify range parsing logic ([f1ed5af](https://github.com/binbandit/snpm/commit/f1ed5af58ad7c9f5a65d0921225edbe176a33cb0))
- *(resolve)* Honor latest dist-tag in selection ([83666c4](https://github.com/binbandit/snpm/commit/83666c4bc64d2508549102c4c7d25b51bcd9661e))
- *(registry)* Align jsr fetch with npm flow ([5574724](https://github.com/binbandit/snpm/commit/5574724d9150d898f30cc9d17bb2b7264bdffc0a))
- *(config)* Handle both authToken key formats ([feac967](https://github.com/binbandit/snpm/commit/feac9679b91b3532bd3e09c199bc2b6b956614dd))
- *(cli)* Use workspace lockfile for upgrades ([470f052](https://github.com/binbandit/snpm/commit/470f0520531000880152effb376889a8c67068e9))
- Resolve clippy warnings and relax pre-release checks ([73eff64](https://github.com/binbandit/snpm/commit/73eff646675c1ce7af916a7c5a00314803c788d4))
- Format snpm-semver with proper line breaks ([2abb4f9](https://github.com/binbandit/snpm/commit/2abb4f9c81f3fcb23adb22721d3d0baa7f916336))
- Rename binary output to snpm instead of snpm-cli ([f271707](https://github.com/binbandit/snpm/commit/f27170754dd49bef39fa6744b46acb3626066692))
- Use rustls-tls instead of default-tls to avoid OpenSSL dependency ([7e2d2da](https://github.com/binbandit/snpm/commit/7e2d2dacbdaafb94d6606fdbac9f85620906f75f))
- *(search)* Use static search handler and config ([bdc653d](https://github.com/binbandit/snpm/commit/bdc653d5cfd95f1951f6d0243ca9943b35394f2c))
- *(home-page)* Respect base path in hero images ([2f3c7ce](https://github.com/binbandit/snpm/commit/2f3c7ce3f67de72ed371120e44817691c658708a))
- *(install)* Link workspace deps with main linker ([773bc30](https://github.com/binbandit/snpm/commit/773bc307bb8db9de9e19122c22e1caab1ae4fc16))
- *(install)* Ensure parent dir for scoped deps ([05b31c9](https://github.com/binbandit/snpm/commit/05b31c95d712c6b28895370334d2905d4902eccd))
- *(linker)* Ensure parent dir and skip copy when no deps ([eb8d51f](https://github.com/binbandit/snpm/commit/eb8d51fa482a9110d27055414b3d28236f404dfb))
- *(core)* Adjust registry URL parsing and auth handling ([982b7ab](https://github.com/binbandit/snpm/commit/982b7abc18f4ea76482617b43b3c7f847d9232a7))
- *(resolve)* Handle non-string registry time field ([d817f28](https://github.com/binbandit/snpm/commit/d817f2871f829fb0c661930f066225bff933fcf1))
- *(linker)* Recurse into node_modules dirs ([0467c12](https://github.com/binbandit/snpm/commit/0467c122ca15eac83a08046487b2d1c1b7fa54bd))
- *(core)* Update protocols module entry point ([6e653c6](https://github.com/binbandit/snpm/commit/6e653c652322186498bb91da7c29e3a571505fdb))

### <!-- 2 -->♻️ Refactor

- *(install)* Remove unused overrides and parser ([8a3ac5a](https://github.com/binbandit/snpm/commit/8a3ac5a12967215b9b98cf83d086d0e12236dddf))
- *(install)* Simplify manifest protocol detection ([d3e8b06](https://github.com/binbandit/snpm/commit/d3e8b067c88cbd449992148742e793b9e2321a19))
- *(cli)* Simplify workspace project loop ([471158d](https://github.com/binbandit/snpm/commit/471158d6f5210e4a4eef9c8bcd8bab0f888517e7))
- *(resolve)* Remove unused normalize_peer_range ([dfc9746](https://github.com/binbandit/snpm/commit/dfc9746b6bc85d4a3e4fddaf9e8fb8524ba82e56))
- *(resolve)* Pass shared http client through ([70f50d8](https://github.com/binbandit/snpm/commit/70f50d88d70b72cf247faeb1b1dd6f26f8793165))
- *(install)* Simplify async prefetch flow ([6ea5562](https://github.com/binbandit/snpm/commit/6ea5562abd83b7134ad48ceb7939e9d6124a37b2))
- *(home)* Remove dynamic version API usage ([7d92968](https://github.com/binbandit/snpm/commit/7d92968917b6b3d32bf918abc439dd12603d1ba8))
- *(home)* Inline acronym list and remove api ([657224c](https://github.com/binbandit/snpm/commit/657224cf50368a18b13b3c8090bd19d9d19c00ed))
- *(console)* Remove unused percent calculation ([f131a0f](https://github.com/binbandit/snpm/commit/f131a0ff7925430fcedcfecaa622c40f27413402))
- *(console)* Remove unused elapsed_ms helper ([fec70ba](https://github.com/binbandit/snpm/commit/fec70bafc1c4e101a01dc49bb8088323a6403532))
- *(install)* Optimize cache and lock reuse ([5696062](https://github.com/binbandit/snpm/commit/5696062a6755c8400e5489b51f64e3b7f5596517))
- *(core)* Decompose registry module into types and logic ([66fe48c](https://github.com/binbandit/snpm/commit/66fe48c81e1c7b3dcba54380a2dcf00cec62b8f2))
- *(core)* Decompose resolve module and extract platform/version logic ([d1b3c94](https://github.com/binbandit/snpm/commit/d1b3c9442178bbfeb0f4df448107ba2efba9f23a))
- *(core)* Decompose linker module into bins, hoist, and fs submodules ([0801a8e](https://github.com/binbandit/snpm/commit/0801a8eca71bd102a91df704f704ec3f65b7279e))
- *(core)* Decompose workspace module into types and discovery ([cafd313](https://github.com/binbandit/snpm/commit/cafd31341662c73cab6f9efcbe8e3b5ea219dfd0))
- *(core)* Normalize let-chain formatting ([7adfb48](https://github.com/binbandit/snpm/commit/7adfb487a0e9be8922daeb73c1598c1acdecaa29))

### <!-- 3 -->📚 Documentation

- *(readme)* Add project overview and usage ([1c99bf3](https://github.com/binbandit/snpm/commit/1c99bf3cd58d4a8bddf819bed1e4c3003e2a8694))
- *(readme)* Document new workspace features ([0a35b0f](https://github.com/binbandit/snpm/commit/0a35b0f6ec91d33c928db33ce53e49b820a65d9a))
- *(readme)* Add centered logo to project intro ([cf4391a](https://github.com/binbandit/snpm/commit/cf4391aa755b83d3a940facf76e63a529ec7d986))
- *(readme)* Enlarge logo for better visibility ([1ef4018](https://github.com/binbandit/snpm/commit/1ef401891018a9851cfccf03354ebb49fae8bf16))
- *(readme)* Remove redundant title heading ([43976c1](https://github.com/binbandit/snpm/commit/43976c17c75852df50e61d4d8391df99aea9c3f7))
- *(release)* Update repo URLs in install steps ([99c4957](https://github.com/binbandit/snpm/commit/99c4957bfe7f0ffe265e3b8043ce4a8fb1cea0de))
- *(config)* Expand env vars and hoisting options ([2b6a982](https://github.com/binbandit/snpm/commit/2b6a982363d456ab47501fb7c0441ce49d8d1083))
- *(core)* Document Bearer/Basic auth schemes and registry URL normalization ([0eb4cd7](https://github.com/binbandit/snpm/commit/0eb4cd724262a2c6fbd25a29f73a1855a7b12da0))
- *(agents)* Add architecture and agent guide ([2da12c6](https://github.com/binbandit/snpm/commit/2da12c6da07abb50901cff9498d171a5508e2434))
- Document protocols, lifecycle and bundled deps ([fedf2d1](https://github.com/binbandit/snpm/commit/fedf2d1ec6241d3adc8d68a419832b786a46dbda))

### <!-- 4 -->⚡ Performance

- Cache registry packages during resolution ([93a93ba](https://github.com/binbandit/snpm/commit/93a93babcbe5773dae6e60b1460facfe8930b696))
- *(install)* Skip scripts when lockfile warm path ([0ec1b65](https://github.com/binbandit/snpm/commit/0ec1b651d7d309bfeef1f0f85dc4b716df267203))

### <!-- 5 -->🎨 Styling

- *(core)* Tidy imports and logging formatting ([ed2dff4](https://github.com/binbandit/snpm/commit/ed2dff416b246dc4396ec2ce37615f3160dc5001))
- *(console)* Improve install output styling ([0ceecff](https://github.com/binbandit/snpm/commit/0ceecfffbea8da06c94dab7e838694a2d0b46e09))
- Remove redundant logging checks for cleaner code ([ee6266b](https://github.com/binbandit/snpm/commit/ee6266b683e328e77c49e685690a6bb50f20e6db))
- *(ops)* Simplify auth filter and reformat exports ([ec80f97](https://github.com/binbandit/snpm/commit/ec80f9719be1209be4f1dc292fb4f30dd59878f3))

### <!-- 6 -->✅ Testing

- *(workspace)* Inline catalog overlay test ([5cd36e2](https://github.com/binbandit/snpm/commit/5cd36e237389383dc9c45b8e781d94c186151a72))
- *(semver)* Add coverage for range parsing ([a585873](https://github.com/binbandit/snpm/commit/a58587369ed670ccd427664b66d513feb0faf6b6))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Bootstrap snpm workspace ([e1eebf3](https://github.com/binbandit/snpm/commit/e1eebf32bd02317f466b45d2cb541eb1d88830d0))
- Tidy formatting and imports ([a69623f](https://github.com/binbandit/snpm/commit/a69623f0a9b5652961330c37566673f29489535d))
- *(gitignore)* Ignore package manager benchmarks ([79e968c](https://github.com/binbandit/snpm/commit/79e968c0c7748613730f2bfed2ff6a9cc205ae2f))
- Format code with cargo fmt ([4019cc0](https://github.com/binbandit/snpm/commit/4019cc0f070bc367072e0495476caaab6b129751))
- *(release)* Bump version to 2025.12.2 [skip ci] ([f133c33](https://github.com/binbandit/snpm/commit/f133c333021df6a20a70ecdb951c1e79c1dae56e))
- *(release)* Add OIDC token and use npm default auth ([3692ff9](https://github.com/binbandit/snpm/commit/3692ff909eb4537013eff7309388ceafd8b2dcd2))
- *(workflow)* Grant id-token write for releases ([6db3836](https://github.com/binbandit/snpm/commit/6db38369c2d03d84521f86cef2df283c7af00994))
- *(release)* Bump version to 2025.12.3 [skip ci] ([a68e9ef](https://github.com/binbandit/snpm/commit/a68e9ef3b5fd418ca48255d9d787ce2cecc437e3))
- *(release)* Refine version bump and push flow ([7cf5b5b](https://github.com/binbandit/snpm/commit/7cf5b5b004982c01ea7f82e126f222cac34c7f01))
- *(docs-deploy)* Relax pnpm lockfile and peer install ([4edb95c](https://github.com/binbandit/snpm/commit/4edb95c1f0062c66365e12e8d7dc91e61b1dc624))
- *(docs-config)* Set export output and base path ([1aa920a](https://github.com/binbandit/snpm/commit/1aa920a02450690463da0117b111645f1e8a8aaf))
- *(release)* Bump version to 2025.12.5 [skip ci] ([0cfcdfd](https://github.com/binbandit/snpm/commit/0cfcdfd59b2971238171d697beb4c3342befca86))
- *(release)* Bump version to 2025.12.6 [skip ci] ([8cf4a69](https://github.com/binbandit/snpm/commit/8cf4a69dfb56e6e342d269cf706b2f36944239f2))
- *(release)* Bump version to 2025.12.7 [skip ci] ([49ed642](https://github.com/binbandit/snpm/commit/49ed642f0a786cff4b041ee8c65154abb486f543))
- *(gitignore)* Exclude e2e2 test directory ([485f023](https://github.com/binbandit/snpm/commit/485f0232bcd50504d8f1655a227092003bb210ab))
- *(release)* Enable npm provenance for publishes ([33dc45b](https://github.com/binbandit/snpm/commit/33dc45b216e60dacd9a1a10b8cc80ed80527dc0b))
- *(taskrunner)* Add just recipes for build/install ([8df3db7](https://github.com/binbandit/snpm/commit/8df3db77dd2cb878981f842465d7f32069620c4a))

<!-- generated by git-cliff -->
