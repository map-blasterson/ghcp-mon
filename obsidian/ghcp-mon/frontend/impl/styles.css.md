---
type: impl
source: web/src/styles.css
lang: css
tags:
  - impl/original
  - impl/css
---
Original source file for reverse-engineered requirements.

This CSS file defines the visual rules referenced by class names from the React layer. The classes `tb-match`, `tb-match-current`, `tb-search-icon`, `tb-search-input`, `tb-search-input-wrap`, `tb-search-header`, `tb-block`, `ib-prim-v-clip`, `ib-prim-v-clip.open`, `ib-prim-k`, `ib-target-arrow`, `kc-cursor-icon`, `ib-mode-toggle`, `ib-mode-btn`, `ib-diff-add`, `ib-diff-rem`, `ib-diff-eq`, `ib-badge-changed`, `ib-badge-added`, `ib-badge-removed`, etc. are visual contracts of the LLRs below.

## Source For
- [[TextBlock cursor-following search hint icon]]
- [[TextBlock click activates search input]]
- [[TextBlock highlights case-insensitive matches]]
- [[TextBlock current match scrolls into view]]
- [[TextBlock truncatable controlled by open prop]]
- [[Chat detail long primitives click to expand]]
- [[Chat detail tool-call hint auto-expand and arrow]]
- [[Chat detail key cursor icon follows pointer]]
- [[Chat detail mode toggle DELTA FULL]]
- [[Chat detail DELTA diffs against prior chat span]]
- [[Skill name chip shows skill argument]]
- [[TextBlock search header shows match counter]]
