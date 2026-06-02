
# Changelog

## [Unreleased]

This release has an [MSRV][] of 1.88.

## [0.9.0][] - 2026-05-15

This release has an [MSRV][] of 1.88.

### Added

- Breaking change in `vello_encoding`: `GlyphRun` now has a `font_embolden` field. Use `FontEmbolden::default()` to preserve the previous behavior. `vello` now re-exports `FontEmbolden` and exposes `DrawGlyphs::font_embolden` for synthetic font emboldening. ([#1628][] by [@jrmoulton][])
- Breaking change in `vello_encoding`: `GlyphRun` now has a `brush_transform` field. Use `None` to preserve the previous behavior. `vello` now exposes `DrawGlyphs::brush_transform` for transforming gradient and image brush contents independently from glyph geometry. ([#1632][] by [@waywardmonkeys][])

### Changed

- Breaking change: `wgpu` has been updated to v29. ([#1534][] by [@nicoburns][])
- Updated `peniko` to v0.6.1, which also updates `kurbo` to v0.13.1 and `color` to v0.3.3. ([#1643][] by [@waywardmonkeys][])
- Updated `skrifa` to v0.42, adding support for VARC glyphs. ([#1594][] by [@nicoburns][] and [@oscargus][])
- `ImageQuality::High` now uses bicubic image sampling. ([#1557][] by [@waywardmonkeys][])
- Image atlas residency is now preserved across renders, avoiding repeated atlas rebuilds and uploads for images that are already resident. ([#1558][] by [@waywardmonkeys][])

### Fixed

- Blurry image rendering due to incorrect half-pixel offset. ([#1606][] by [@Keavon][] and [@xStrom][])
- Inactive `clip_leaf` shader lanes no longer perform invalid shared-memory reads, fixing black frames for some clip-layer scenes on Android/Vulkan. ([#1637][] by [@gugutu][])
- Override image atlas entries are now marked dirty when override textures are inserted, removed, or explicitly refreshed with `Renderer::mark_override_image_dirty`. ([#1638][] by [@waywardmonkeys][] and [@raphlinus][])

## [0.8.0][] - 2026-03-20

This release has an [MSRV][] of 1.92.

### Changed

- Breaking change: `wgpu` has been updated to v28. ([#1492][] by [@xStrom][])

[MSRV]: README.md#minimum-supported-rust-version-msrv

[Unreleased]: https://github.com/linebender/vello/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/linebender/vello/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/linebender/vello/compare/v0.7.0...v0.8.0

[@raphlinus]: https://github.com/raphlinus
[@gugutu]: https://github.com/gugutu
[@jrmoulton]: https://github.com/jrmoulton
[@Keavon]: https://github.com/Keavon
[@nicoburns]: https://github.com/nicoburns
[@oscargus]: https://github.com/oscargus
[@waywardmonkeys]: https://github.com/waywardmonkeys
[@xStrom]: https://github.com/xStrom

[#1492]: https://github.com/linebender/vello/pull/1492
[#1534]: https://github.com/linebender/vello/pull/1534
[#1557]: https://github.com/linebender/vello/pull/1557
[#1558]: https://github.com/linebender/vello/pull/1558
[#1594]: https://github.com/linebender/vello/pull/1594
[#1606]: https://github.com/linebender/vello/pull/1606
[#1628]: https://github.com/linebender/vello/pull/1628
[#1632]: https://github.com/linebender/vello/pull/1632
[#1637]: https://github.com/linebender/vello/pull/1637
[#1638]: https://github.com/linebender/vello/pull/1638
[#1643]: https://github.com/linebender/vello/pull/1643
