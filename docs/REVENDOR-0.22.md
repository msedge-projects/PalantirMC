# Re-vendoring the reference from the 0.21.4-era pin to v0.22.0

What changed between `vendor/modrinth-app` and the v0.22.0 tree in
`.scratch/modrinth-0.22/`, what it costs a launcher that must look identical, and the
order to re-run the generators in. Every number was read out of the two trees with
`Get-FileHash`, `Compare-Object` and `python -c`; no cargo and no git were run.

| | |
| --- | --- |
| Old tree | `vendor/modrinth-app`, commit `8966b5e2e7951e83651fbebbf2fb6d7608a94a33`, 2026-09-20 13:33:59 +02:00, per `UPSTREAM.md` |
| New tree | `.scratch/modrinth-0.22/modrinth-code-51e57af/` (`apps/`, `packages/`), from `.scratch/modrinth-0.22.tar.gz`, 255,040,081 bytes |
| Release record | `.scratch/rel22.json` is commit `e7757b14ed04e824ac7e8b8b05652d4290df721a`, "chore: changelog app 0.22.0 (#8073)", 2026-10-21T22:17:07Z; its patch adds the app 0.22.0 entry to `packages/blog/changelog.ts` |
| Diff listing | `.scratch/diff-22.tsv`, 431 rows, `status<TAB>tree<TAB>path`, three trees only |

It carries no tag object, and the commit the tarball was cut from (`51e57af`) cannot be checked against a tag without network or git: **tag-to-commit could not
be determined here**. Paths below use the vendor tree's flattened names; the new tree's
equivalents are `apps/app-frontend`, `packages/ui`, `packages/assets`,
`packages/tooling-config/tailwind`.

## 1. Summary of the file diff

| Tree | Changed (M) | Added (A) | Removed (D) | Net | Files before | Files after |
| --- | --- | --- | --- | --- | --- | --- |
| `app-frontend/` | 97 | 79 | 60 | +19 | 462 | 481 |
| `ui/` | 132 | 39 | 3 | +36 | 833 | 869 |
| `assets/` | 3 | 18 | 0 | +18 | 561 | 579 |
| `tooling-config/` | 0 | 0 | 0 | 0 | 1 | 1 |
| **Total** | **232** | **136** | **63** | **+73** | **1857** | **1930** |

By extension (M / A / D): `.vue` 93 / 74 / 45, `.ts` 63 / 43 / 18, `.json` 69 / 0 / 0,
`.svg` 0 / 18 / 0, `.js` 4 / 0 / 0, `.jsonc` 1 / 0 / 0, `.md` 1 / 1 / 0, `.scss` 1 / 0 / 0.

What matters to a launcher that must look identical:

| Matters | Rows | Why |
| --- | --- | --- |
| `assets/styles/classes.scss` | 1 M | the only stylesheet that changed (section 2) |
| `assets/icons/*.svg`, `assets/generated-icons.ts` | 18 A, 1 M | 18 new glyphs the v0.22 UI draws (section 4) |
| `app-frontend/src/locales/**`, `ui/src/locales/**` | 33 M, 32 M | every visible string (section 5) |
| `.vue` under `app-frontend/src` and `ui/src` | 212 (93 M, 74 A, 45 D) | shell, pages, settings modals, widgets (sections 3, 6) |

Does not matter (no pixels): `ui/src/styles/*.css` and
`tooling-config/tailwind-preset.ts` (0 rows, byte-identical per section 2),
`app-frontend/src/generated/app-events/**` (7 M, 3 D postcard codecs), `package.json`
/ `wrangler.jsonc` / `AGENTS.md` / `README.md`, `ui/src/stories/**` (Storybook only).

## 2. Design tokens

`assets/styles/variables.scss` is **byte-identical** - 14,161 bytes, SHA-256
`FD247116D3BEF1D1991F332CE59CD51460989196149F6B636021C8CCD1AD9794` - and so are the
other eight sheets in `assets/styles/`, both sheets in `ui/src/styles/`, and
`packages/tooling-config/tailwind/tailwind-preset.ts` against
`vendor/modrinth-app/tooling-config/tailwind-preset.ts` (SHA-256
`56A9A2A482926121DDD24126022170F4E2434FB8D3200B625E974C87CE4399A8`); `classes.scss` is the only stylesheet that differs.

Custom properties declared in either tree, read from every `.scss`, `.css` and
`.vue`: **254 definitions in both, 0 value changes, 6 added, 6 removed**, and the
six are one block that moved file without changing a byte:

| Property | Old = new value | Old file | New file |
| --- | --- | --- | --- |
| `--surface-4`, `--color-button-bg` | `var(--brand-gradient-button)` | `app-frontend/src/App.vue` | `app-frontend/src/app/shell/sidebar/index.vue` |
| `--surface-5`, `--color-divider`, `--color-divider-dark`, `--color-button-bg-hover` | `var(--brand-gradient-border)` | same | same |

The scope moves with them - they now apply to the sidebar subtree rather than the app
root, which is why the gradient-brand block left `App.vue`. `classes.scss` (21,380 to
21,344 bytes, 1,199 to 1,195 lines) has two hunks and no token added, removed or
re-pointed:

| Old | New | Effect |
| --- | --- | --- |
| `> :nth-child(2) { @apply mt-5; }` | (deleted) | the block's second child loses 20px of top margin |
| `> summary { @apply border-b-surface-5; }` | `> summary { @apply border-b-surface-5 mb-3; }` | a bordered summary gains 12px below it |

## 3. Transitions, animations and durations

Every `.css`, `.scss`, `.vue`, `.ts` and `.js` file in the three trees was scanned
for `transition`, `animation`, `cubic-bezier`, `duration` and `ease`: 299 files old,
319 new, 282 in both, **12 with changed content**, 37 new-only, 17 old-only; 1,749
matching lines old against 1,823 new.

| File | Old | New |
| --- | --- | --- |
| `ui/src/components/base/Toggle.vue` | track and thumb both `transition-all duration-200` | track `transition-[transform,background-color,width,height,margin] duration-200`, thumb `transition-[background-color,border-color,opacity] duration-200` |
| `ui/src/components/modal/NewModal.vue` | (none) | WAAPI resize `{ duration: 200, easing: 'ease-in-out' }`, cancelled by `resetResizeAnimation()` |
| `ui/src/components/base/Combobox.vue` | `<Transition name="floating-expand">` | `<Transition name="floating-expand" :css="animateDropdown">`; story `WithoutDropdownAnimation` added |
| `ui/src/components/servers/ServerManageStats.vue` | `transition-transform duration-100 hover:brightness-125 active:scale-95` | (removed) |
| `ui/src/components/base/{PageLoadingBar,InlineValidationMessage,SegmentedControl}.vue` (new) | - | `width 0.1s ease-in-out` while finishing and `opacity 0.4s`; `grid-template-rows 150ms ease, opacity 150ms ease, transform 150ms ease`; `left|right 80ms cubic-bezier(0.4, 0, 0.2, 1)`, `background-color 100ms ease` |
| `ui/src/components/modal/update-all-modal/index.vue` (new) | - | `transition: opacity 150ms`; changelog pane `{ duration: noChangelogMotion ? 0 : 0.3, ease: 'easeInOut' }` |
| `ui/src/layouts/wrapped/hosting/manage/backups/history-table.vue` (new) | - | `@keyframes backup-row-highlight` + `animation: backup-row-highlight 1.25s ease-in-out infinite` |
| `ui/src/layouts/wrapped/hosting/manage/backups.vue` (deleted) | `all 200ms ease-in-out`, `transform 200ms ease-in-out`, `all 0.5s ease-out`, `all 0.2s ease-out`, `@keyframes indeterminate` | gone with the file; the replacement directory declares only `backup-row-highlight` |
| `app-frontend/.../settings-modal/settings-options-transition.vue` | `grid-template-rows 0.25s var(--ease-out-expo), opacity 0.2s ease-out`; `transition: none` | identical, moved to `app-frontend/src/features/instances/settings-modal/` |
| `app-frontend/src/components/ui/Breadcrumbs.vue` (deleted) | `@keyframes breadcrumb-scroll` | same keyframes, moved to `app-frontend/src/app/shell/title-bar/breadcrumbs.vue` |

No existing duration changed value. What disappeared went with two rebuilt surfaces:
`backups.vue` (200ms, 200ms, 500ms, 200ms) and `ServerManageStats.vue` (100ms). What
appears is 80ms, 100ms, 150ms, 200ms (the same 200ms G54 already asserts for the
modal), 250ms, 300ms, 400ms and 1.25s, all on
`TAILWIND_DURATION_MS = [0, 75, 100, 150, 200, 300, 500, 700, 1000]` at
`tools/gen_theme.py:137`. Easing tokens are untouched: `--ease-out-expo` is
`cubic-bezier(0.16, 1, 0.3, 1)` in the byte-identical `tailwind-utilities.css`, so the
four curves in `theme_gen.rs` do not move, and the only new curve in the tree is
`cubic-bezier(0.4, 0, 0.2, 1)` in the new `SegmentedControl.vue`. For the +-16ms budget: **every duration this launcher already draws is unchanged.**

## 4. Icons

| Source of truth `assets/icons/` | Old | New | Delta |
| --- | --- | --- | --- |
| Top-level `*.svg` | 313 | 331 | +18, none removed, none renamed |
| `tags/categories/*.svg` | 102 | 102 | 0 |
| `tags/loaders/*.svg` | 30 | 30 | 0 |
| `badges/*.svg` | 20 | 20 | 0 |
| All `.svg` under `icons/` | 465 | 483 | +18 |

No icon file was modified (18 `A`, 0 `M` among `assets/` SVGs), so every glyph the
launcher already strokes keeps its geometry. `assets/generated-icons.ts` goes 1,076 to
1,112 lines (+36 = 18 imports + 18 exports, nothing removed), `tools/gen_icons.py:99`
reads that directory, so `icons_gen.rs` gains those 18 glyphs and nothing else:
`ArrowBigUp`, `CalendarClock`, `CircleDot`, `GitCommitHorizontal`, `LayoutPanelLeft`,
`ListChevronsDownUp`, `ListChevronsUpDown`, `MessageSquareWarning`, `MonitorX`,
`PanelLeftClose`, `PanelLeftOpen`, `PanelRightClose`, `PanelRightOpen`, `PanelTop`,
`PanelTopClose`, `PanelTopOpen`, `Smartphone`, `Tablet`.

## 5. Locales

Thirty-three locale directories in both trees, in both packages; none added, none
removed; every `meta.json` unchanged; `index.json` changed in all 33 `app-frontend`
locales and 32 of 33 `ui` locales.

| | Old keys | New keys | Added | Removed | Values changed |
| --- | --- | --- | --- | --- | --- |
| `app-frontend/src/locales/en-US/index.json` | 1,516 | 1,550 | 57 | 23 | 1 (`instance.shared-instance.publish-review.header.message`) |
| `ui/src/locales/en-US/index.json` | 2,330 | 2,504 | 190 | 16 | 2 (`content.diff-modal.config-files-updated.defaultMessage`, `external-files.permissions-card.editor.select-license-label.defaultMessage`) |
| Both, as `gen_text.py` compiles them | 3,846 | 4,054 | 247 | 39 | 3 |

Across all 33 locales the leaves go 31,475 to 35,726 (`app-frontend`) and 57,702 to
60,086 (`ui`): **89,177 to 95,812**, +6,635. The 89,177 is the figure G120 asserts, and the full added and removed key lists are in the appendix.

## 6. Routes, pages and layout

`app-frontend/src/routes.js` goes 163 to 166 lines and **no route was added or
removed** - same names, paths and components, including `InstanceContent` and
`InstanceContentFilter`, which both mount `@/pages/instance/content/index.vue`. What
changed is scrolling: four routes gained `meta` (`Home` -> `{ manualScroll: true }`;
`Discover content`, `InstanceContent`, `InstanceContentFilter` -> `{ saveScroll:
true }`) and the global `scrollBehavior` was deleted. That hook reset `.app-viewport`
to 0 on any path change except `Home`; it is replaced by the new
`app-frontend/src/app/runtime/use-viewport-scroll-restore.ts`.

| Area | Old | New |
| --- | --- | --- |
| App shell | `src/App.vue`, 2,869 lines, sidebar and title bar inline; `components/ui/{AppActionBar,WindowControls,Breadcrumbs,PromotionWrapper}.vue` | `src/App.vue` 2,449 lines; new `src/app/shell/title-bar/{index,action-bar,breadcrumbs,window-controls}.vue`, `src/app/shell/sidebar/{index,promotion}.vue` |
| Right sidebar | inline `.app-sidebar` | `app/shell/sidebar/index.vue`, 172 lines, fixed `w-[300px]`, `h-[calc(100vh-var(--top-bar-height))]`, edge shadow `-15px 0 15px -15px rgba(0,0,0,0.1) inset` when `appTheme.advancedRendering`, fade from `useScrollIndicator` |
| Nav rail | seven `NavButton`s in `App.vue` | unchanged: `/`, `/browse/modpack`, `/skins` (when `showSkinSelectorInSidebar`), `/screenshots` (when the synced option is on), `/hosting/manage`, create-instance, settings |
| App settings modal | `components/ui/modal/AppSettingsModal.vue`, 392 lines, 11 tabs | `features/settings/app-settings-modal/index.vue`, 442 lines, 14 tabs: `.synced-options` -> `.syncing`, `.java-installations` -> `.java-and-memory`, `.resource-management` -> `.resources`, plus new `.window`, `.launch-hooks`, `.troubleshooting` |
| Instance settings modal | `pages/instance/components/settings-modal/index.vue`, 195 lines, 4 tabs (general, installation, settings-overrides, sharing) | `features/instances/settings-modal/index.vue`, 250 lines, 7 tabs in two labelled groups (`instance.settings.sidebar.label.instance` / `.game`): the four minus `settings-overrides`, plus java, window, hooks, syncing |
| Page files | - | no `src/pages/**` added; 13 deleted (all of `pages/instance/components/settings-modal/`, moved to `features/`); 18 modified, including `instance/layout.vue`, `Index.vue`, `Browse.vue`, `Skins.vue`, `Servers.vue`, `User.vue`, `hosting/manage/Backups.vue`, `project/{Index,Version}.vue`, `instance/content/index.vue` |
| `ui` layouts and widgets | `layouts/wrapped/hosting/manage/backups.vue` | 12 layouts added, 1 removed: `backups/{index,history-table,slot-card,use-focused-backup}`, `shared/server-settings/pages/{backups,support}.vue`, `shared/user-profile/components/{restrict-user,force-password-reset,lock-user,reset-2fa}-modal.vue`, `shared/installation-settings/{ContentDiffList.vue,utils.ts}`; 17 widgets added, including `base/{InlineValidationMessage,SegmentedControl,FullImage,PageLoadingBar}.vue`, `modal/update-all-modal/*` (9 files), `servers/backups/*`, `servers/{LockServerModal,ServerStatCard}.vue` |

## 7. Re-vendor plan for this repository

Five tools read the vendored tree; a sixth mentions it only in prose:

| Tool | What it reads | What it writes |
| --- | --- | --- |
| `tools/gen_theme.py` | `VENDOR` (line 67), `assets/styles/variables.scss` (line 70), `ui/src/styles/tailwind-utilities.css`, `MOTION_DIRS` = `assets/styles`, `ui/src`, `app-frontend/src` (lines 464-468) | `crates/palantir-desktop/src/theme_gen.rs` |
| `tools/gen_icons.py` | `ICONS = vendor/modrinth-app/assets/icons` (line 99) | `.../icons_gen.rs` |
| `tools/gen_text.py` | the two `en-US/index.json` (lines 73-74) | `.../text_gen.rs` |
| `tools/gen_tokens.py` | `assets/styles/{variables,defaults}.scss` (lines 47-48) | `.../theme_tokens.rs` |
| `tools/gen_locale.py` | the two `locales/` directories (lines 84-85); imports `gen_text` | `.../locale_gen.rs` |

`tools/lucide_source.py` names this tree only in its docstring (line 5); it reads and
writes `vendor/lucide-icons/`, a different tree. `reference_tokens.rs` is not
generated either: it is the independent reader G46-G52 assert against.

### Order

GATES.md records the chain at line 1781 as `gen_theme.py --check && gen_icons.py
--check && gen_text.py --check && gen_tokens.py --check`. Keep that order, and put
the locale compiler after `gen_text.py`, whose module it imports.

1. Copy the new tree into `vendor/modrinth-app` **keeping the flattened layout**
   (`apps/app-frontend` -> `app-frontend/`, `packages/ui` -> `ui/`, `packages/assets`
   -> `assets/`, `packages/tooling-config/tailwind/tailwind-preset.ts` ->
   `tooling-config/tailwind-preset.ts`): the five readers hard-code those flat paths,
   so a copy that keeps `apps/` and `packages/` breaks all of them. Rewrite
   `UPSTREAM.md` in the same commit (481 / 869 / 579 / 1 files, 11.8 / 10.8 / 4.5 / 0
   MB, 1,930 total against the recorded 1,857).
2. `python tools/gen_theme.py` then `--check`. Expect the motion tables to change and
   nothing else: `theme_gen.rs` records each declaration's path and text, and lines
   1087-1088 name the moved `settings-options-transition.vue` under
   `app-frontend/src/pages/instance/components/settings-modal/`, a path that no longer
   exists, so the moved transition, the moved breadcrumb keyframes and every entry
   added or dropped in section 3 rewrite lines. Palette, radii, gaps and curves cannot
   move: their sheets are byte-identical.
3. `python tools/gen_icons.py` then `--check`: +18 glyphs (313 -> 331 top-level SVGs).
4. `python tools/gen_text.py` then `--check`: 3,846 -> 4,054 messages, so the `Key`
   enum, the ICU helpers and every plural arm change.
5. `python tools/gen_tokens.py` then `--check`. Expect **byte-identical**: it reads
   only `variables.scss` and `defaults.scss`, both unchanged, so 189 tokens in each of
   four modes stay 189.
6. `python tools/gen_locale.py`, then `--check` and `--report`: 33 trees carrying
   89,177 -> 95,812 leaves, so the sparse tables and the coverage ledger both change.

### Gates that came from the old tree

GATES.md holds 171 checked gates. These are the ones whose expectations are statements
about the vendored tree, and what a re-vendor does to each:

| Gate | What it asserts from the old tree | After re-vendor |
| --- | --- | --- |
| G46-G50 (`reference_tokens.rs`) | 72 transcribed values, 24 declared deviations, 3 modes, the radius constants, the text-size ladder, the weight rungs | re-run only; no palette value changed |
| G51, G52 (`theme_tokens.rs`) | 189 tokens x 4 modes; the 21 files outside the two global sheets, keyed `file:line` | values unchanged, but G52's keys are `file:line`, so the moved `settings-options` block changes keys |
| G54, G55 (`anim.rs`) | the 200ms modal deadline, cited from `NewModal.vue`'s `transition: all 0.2s` | value survives; that file now also carries a 200ms `ease-in-out` ResizeObserver animation, so the citation must be widened |
| G59 (`motion.rs`) | the reference's five curves, sampled by `tools/curve_samples.html` | unchanged |
| G60 (`gen_text.py`) | 1,516 + 2,330 = 3,846 messages | **re-derive**: 1,550 + 2,504 = 4,054 |
| G120 (`gen_locale.py`) | 33 trees, 89,177 leaves, 2,700,238 bytes of translated text | **re-derive**: 33 trees, 95,812 leaves; the byte figure could not be determined without `--report` |
| G122 (`gen_locale.py --report`) | 126,918 pairs = 33 x 3,846, 70.3% filled, th-TH 459 (11.9%), de-CH 3,792 (98.6%), 37,741 pairs (29.7%) falling back to English, 244 names in all 32 | **re-derive**: 133,782 pairs = 33 x 4,054; that arithmetic over the measured leaves gives 71.6%, th-TH 464 (11.4%), de-CH 3,917 (96.6%); the fallback and "all 32" counts need `--report` |
| G125 | an instance's settings carry the heap and Java path controls | re-take the capture: that dialog is now 7 tabs in two groups |
| G143, G148, G170, G171 | `tag.loader.bta-babric`; `size-5` and 1em glyph sizes; one tessellation per glyph key; no per-frame geometry rebuild | `bta-babric` still present in the new `ui` en-US, one occurrence either side; icon rules unchanged, the set grows 313 -> 331 |

The counters inside G143, G148 and G170 ("1281 passed", "1284 passed", "893 passed")
are test counts on the old generated tables and move with them; they record a run, they are not oracles. Every other gate measures this launcher's own capture against
numbers taken from the old tree's pixels, and those hold only where the chrome did not
move: the tokens did not (section 2), so the colour, radius and type gates stand, while
any gate sampling a settings dialog, a sidebar or a hosting page must be re-taken.

## Appendix: every locale key added and removed

<details>
<summary>app-frontend/src/locales/en-US/index.json: 57 added, 23 removed</summary>
Added (57):
app.action-bar.install.summary.instance-error.message; app.behavior-settings.refocus-on-game-close.description.message; app.behavior-settings.refocus-on-game-close.title.message; app.debug-export.canceling.message; app.debug-export.exporting.message; app.debug-export.finishing.message; app.debug-export.preparing.message; app.download-manager.bulk-update-content.message; app.file-download.canceling.message; app.file-download.preparing.message; app.file-download.waiting.message; app.modal.install-to-play.unavailable-description.message; app.modal.install-to-play.unavailable-header.message; app.settings.resource-management.debug-info.description.message; app.settings.resource-management.debug-info.export.message; app.settings.resource-management.debug-info.title.message; app.settings.sidebar.label.instance-defaults.message; app.settings.sidebar.label.system.message; app.settings.tabs.java-and-memory.message; app.settings.tabs.launch-hooks.message; app.settings.tabs.resources.message; app.settings.tabs.syncing.message; app.settings.tabs.troubleshooting.message; app.settings.tabs.window.message; app.settings.troubleshooting.clear-cache.confirm.title.message; app.settings.troubleshooting.clear-cache.message; app.settings.troubleshooting.report-ad.copied.message; app.settings.troubleshooting.report-ad.copy.message; app.settings.troubleshooting.report-ad.description.message; app.settings.troubleshooting.report-ad.no-ad.text.message; app.settings.troubleshooting.report-ad.no-ad.title.message; app.settings.troubleshooting.report-ad.title.message; app.shared-instance-publish.preparing.message; app.shared-instance-publish.pushed.message; app.shared-instance-publish.uploading.message; app.skins.section.dungeons-hero.message; app.skins.section.dungeons-ii-hero.message; app.skins.section.wilderness-bound.message; instance.settings.defaults.open-app-settings.message; instance.settings.sidebar.label.game.message; instance.settings.sidebar.label.instance.message; instance.settings.tabs.hooks.default-settings-description.message; instance.settings.tabs.hooks.label.message; instance.settings.tabs.java.default-settings-description.message; instance.settings.tabs.java.label.message
instance.settings.tabs.syncing.message; instance.settings.tabs.window.default-settings-description.message; instance.settings.tabs.window.label.message; instance.shared-instance.file-not-allowed.title.message; instance.shared-instance.publish-review.changes-description.message; instance.shared-instance.publish-review.changes-tab.message; instance.shared-instance.publish-review.file-type-not-allowed.message; instance.shared-instance.publish-review.files-description.message; instance.shared-instance.publish-review.files-tab.message; instance.shared-instance.publish-review.storage-over-limit.message; instance.shared-instance.publish-review.storage-title.message; instance.shared-instance.publish-review.storage-usage.message
Removed (23):
app.action-bar.install.summary.instance-not-found.message; app.instance.mods.bulk-update.downloading-projects.message; app.instance.mods.bulk-update.finishing.message; app.instance.mods.bulk-update.resolving-versions.message; app.settings.default-instance-options.java-and-memory.title.message; app.settings.default-instance-options.launch-hooks.title.message; app.settings.default-instance-options.window.title.message; app.settings.resource-management.app-cache.confirm.title.message; app.settings.resource-management.app-cache.purge.message; app.settings.resource-management.store.empty.message; app.settings.sidebar.label.instances.message; app.settings.tabs.resource-management.message; app.settings.tabs.synced-options.message; instance.settings.tabs.hooks.description.message; instance.settings.tabs.java.custom-environment-variables.message; instance.settings.tabs.java.custom-java-arguments.message; instance.settings.tabs.java.custom-java-installation.message; instance.settings.tabs.java.custom-memory-allocation.message; instance.settings.tabs.settings-overrides.message; instance.settings.tabs.window.custom-window-settings.message; instance.shared-instance.publish-review.admonition-header.message; instance.shared-instance.publish-review.config-title-v2.message; instance.shared-instance.publish-review.description.message
</details>

<details>
<summary>ui/src/locales/en-US/index.json: 190 added, 16 removed</summary>
Added (190):
content.page-layout.failed-to-load-changelog.defaultMessage; content.page-layout.failed-to-load-updates.defaultMessage; external-files.permissions-card.file-database-id.defaultMessage; external-files.permissions-card.license.all-rights.defaultMessage; hosting.save-banner.resolve-errors.defaultMessage; hosting.save-banner.saved.defaultMessage; hosting.settings.properties.motd-profanity-save-error.defaultMessage; hosting.settings.properties.motd-profanity.defaultMessage; profile.button.edit-restrictions.defaultMessage; profile.button.edit-user.defaultMessage; profile.button.force-password-reset.defaultMessage; profile.button.lock.defaultMessage; profile.button.reset-2fa.defaultMessage; profile.button.restrict.defaultMessage; profile.button.revoke-sessions.defaultMessage; profile.button.unlock.defaultMessage; profile.force-password-reset.admonition-body.defaultMessage; profile.force-password-reset.admonition-title.defaultMessage; profile.force-password-reset.button.defaultMessage; profile.force-password-reset.email-label.defaultMessage; profile.force-password-reset.email-placeholder.defaultMessage; profile.force-password-reset.error-description.defaultMessage; profile.force-password-reset.error-title.defaultMessage; profile.force-password-reset.success-description.defaultMessage; profile.force-password-reset.success-title.defaultMessage; profile.force-password-reset.title.defaultMessage; profile.label.locked.defaultMessage; profile.label.restricted.defaultMessage; profile.reset-2fa.admonition-body.defaultMessage; profile.reset-2fa.admonition-title.defaultMessage; profile.reset-2fa.button.defaultMessage; profile.reset-2fa.error-description.defaultMessage; profile.reset-2fa.error-title.defaultMessage; profile.reset-2fa.success-description.defaultMessage; profile.reset-2fa.success-title.defaultMessage; profile.reset-2fa.title.defaultMessage; profile.restrict-user.admonition-body.defaultMessage; profile.restrict-user.admonition-title.defaultMessage; profile.restrict-user.error-description.defaultMessage; profile.restrict-user.header.defaultMessage; profile.restrict-user.private-reason-label.defaultMessage; profile.restrict-user.private-reason-placeholder.defaultMessage; profile.restrict-user.reason-label.defaultMessage; profile.restrict-user.reason-placeholder.defaultMessage; profile.restrict-user.remove-button.defaultMessage
profile.restrict-user.remove-error-title.defaultMessage; profile.restrict-user.remove-success-description.defaultMessage; profile.restrict-user.remove-success-title.defaultMessage; profile.restrict-user.save-button.defaultMessage; profile.restrict-user.save-error-title.defaultMessage; profile.restrict-user.save-success-description.defaultMessage; profile.restrict-user.save-success-title.defaultMessage; profile.revoke-sessions.admonition-body.defaultMessage; profile.revoke-sessions.admonition-title.defaultMessage; profile.revoke-sessions.button.defaultMessage; profile.revoke-sessions.error-description.defaultMessage; profile.revoke-sessions.error-title.defaultMessage; profile.revoke-sessions.success-description.defaultMessage; profile.revoke-sessions.success-title.defaultMessage; profile.revoke-sessions.title.defaultMessage; profile.unlock-user.error-description.defaultMessage; profile.unlock-user.error-title.defaultMessage; profile.unlock-user.success-description.defaultMessage; profile.unlock-user.success-title.defaultMessage; project.settings.environment.bulk.apply-button.defaultMessage; project.settings.environment.bulk.description.defaultMessage; project.settings.environment.bulk.title.defaultMessage; project.settings.environment.modal.title.defaultMessage; servers.access-page.activity-log-filter.action.sftp-credentials-rolled.defaultMessage; servers.audit-log.event.sftp-credentials-rolled.defaultMessage; servers.backups.create.in-progress.defaultMessage; servers.backups.create.slots-full.defaultMessage; servers.backups.delete.error.defaultMessage; servers.backups.history.auto-badge.defaultMessage; servers.backups.history.column.created-by.defaultMessage; servers.backups.history.column.created.defaultMessage; servers.backups.history.column.name.defaultMessage; servers.backups.history.creator-avatar-alt.defaultMessage; servers.backups.history.creator.support.defaultMessage; servers.backups.history.more-options.defaultMessage; servers.backups.history.no-results.defaultMessage; servers.backups.history.rename.defaultMessage; servers.backups.history.restore.defaultMessage; servers.backups.history.search-label.defaultMessage; servers.backups.history.search-placeholder.defaultMessage; servers.backups.history.title.defaultMessage; servers.backups.interval.days.defaultMessage; servers.backups.interval.every-days.defaultMessage; servers.backups.interval.every-hours.defaultMessage; servers.backups.interval.hours.defaultMessage
servers.backups.interval.weekly.defaultMessage; servers.backups.load-error.description.defaultMessage; servers.backups.load-error.title.defaultMessage; servers.backups.restore.operation-in-progress.defaultMessage; servers.backups.restore.server-running.defaultMessage; servers.backups.rotation-modal.admonition-body.defaultMessage; servers.backups.rotation-modal.admonition-header.defaultMessage; servers.backups.rotation-modal.backups-label.defaultMessage; servers.backups.rotation-modal.confirm.defaultMessage; servers.backups.rotation-modal.header.defaultMessage; servers.backups.slots.auto.next-soon.defaultMessage; servers.backups.slots.auto.next.defaultMessage; servers.backups.slots.auto.off-description.defaultMessage; servers.backups.slots.auto.off.defaultMessage; servers.backups.slots.auto.title.defaultMessage; servers.backups.slots.limit.defaultMessage; servers.backups.slots.manual.description.defaultMessage; servers.backups.slots.manual.title.defaultMessage; servers.backups.toolbar.more-create-options.defaultMessage; servers.files.server-locked.defaultMessage; servers.lock.modal.error-text.defaultMessage; servers.lock.modal.error-title.defaultMessage; servers.lock.modal.header.defaultMessage; servers.lock.modal.lock-button.defaultMessage; servers.lock.modal.reason-label.defaultMessage; servers.lock.modal.reason-placeholder.defaultMessage; servers.lock.modal.success-text.defaultMessage; servers.lock.modal.success-title.defaultMessage; servers.lock.modal.warning-body.defaultMessage; servers.lock.modal.warning-header.defaultMessage; servers.manage.lock-server.defaultMessage; servers.manage.locked.badge-tooltip.defaultMessage; servers.manage.locked.badge.defaultMessage; servers.manage.locked.body.defaultMessage; servers.manage.locked.header.defaultMessage; servers.manage.unlock-server.defaultMessage; servers.manage.unlock.error-text.defaultMessage; servers.manage.unlock.error-title.defaultMessage; servers.manage.unlock.success-text.defaultMessage; servers.manage.unlock.success-title.defaultMessage; servers.region.out-of-stock.defaultMessage; servers.region.ping.failed.defaultMessage; servers.region.ping.latency.defaultMessage; servers.region.ping.testing.defaultMessage; servers.settings.backups.auto.description.defaultMessage
servers.settings.backups.auto.title.defaultMessage; servers.settings.backups.interval.title.defaultMessage; servers.settings.backups.manual-slots.description.defaultMessage; servers.settings.backups.manual-slots.title.defaultMessage; servers.settings.backups.save.error-title.defaultMessage; servers.settings.support.action-log-title.defaultMessage; servers.settings.support.clear-action-log-button.defaultMessage; servers.settings.support.clear-action-log-confirmation.defaultMessage; servers.settings.support.clear-action-log-description.defaultMessage; servers.settings.support.clear-action-log-error.defaultMessage; servers.settings.support.clear-action-log-success.defaultMessage; servers.settings.support.clear-action-log-title.defaultMessage; update-all-modal.changelog-for.defaultMessage; update-all-modal.changelog-loaded.defaultMessage; update-all-modal.close-changelog.defaultMessage; update-all-modal.current-version.defaultMessage; update-all-modal.empty.defaultMessage; update-all-modal.header.defaultMessage; update-all-modal.loading-changelog.defaultMessage; update-all-modal.loading.defaultMessage; update-all-modal.new-version.defaultMessage; update-all-modal.no-changelog.defaultMessage; update-all-modal.no-compatible-updates.defaultMessage; update-all-modal.no-version.defaultMessage; update-all-modal.project-list.defaultMessage; update-all-modal.project.defaultMessage; update-all-modal.results.defaultMessage; update-all-modal.select-all.defaultMessage; update-all-modal.select-project.defaultMessage; update-all-modal.select-version.defaultMessage; update-all-modal.update.defaultMessage; update-all-modal.versions.defaultMessage; update-all-modal.view-changelog.defaultMessage; update-all-modal.view.defaultMessage; update-all-modal.warning-server.defaultMessage; update-all-modal.warning.defaultMessage; user-restrictions.group.collections.defaultMessage; user-restrictions.group.create-projects.defaultMessage; user-restrictions.group.edit-projects.defaultMessage; user-restrictions.group.organizations.defaultMessage; user-restrictions.group.profile.defaultMessage; user-restrictions.group.versions.defaultMessage; user-restrictions.scope.collection-create.defaultMessage; user-restrictions.scope.collection-delete.defaultMessage; user-restrictions.scope.collection-write.defaultMessage
user-restrictions.scope.organization-create.defaultMessage; user-restrictions.scope.organization-delete.defaultMessage; user-restrictions.scope.organization-write.defaultMessage; user-restrictions.scope.project-create.defaultMessage; user-restrictions.scope.project-delete.defaultMessage; user-restrictions.scope.project-write.defaultMessage; user-restrictions.scope.user-write.defaultMessage; user-restrictions.scope.version-create.defaultMessage; user-restrictions.scope.version-delete.defaultMessage; user-restrictions.scope.version-write.defaultMessage
Removed (16):
hosting.loader.support-options-title.defaultMessage; servers.backups.bulk-bar.aria-label.defaultMessage; servers.backups.bulk-bar.deleting.defaultMessage; servers.backups.filtered-empty.clear-filters.defaultMessage; servers.backups.filtered-empty.description.defaultMessage; servers.backups.filtered-empty.heading.defaultMessage; servers.backups.item.backup-schedule.defaultMessage; servers.backups.item.creator-avatar-alt.defaultMessage; servers.backups.item.creator.support.defaultMessage; servers.backups.item.manual-backup.defaultMessage; servers.backups.item.rename.defaultMessage; servers.backups.item.restore.defaultMessage; servers.backups.select-backup-aria.defaultMessage; servers.backups.toolbar.filter-auto.defaultMessage; servers.backups.toolbar.filter-manual.defaultMessage; servers.backups.toolbar.select-all.defaultMessage
</details>
