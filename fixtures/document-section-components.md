---
title: Section components fixture
sections:
  timeline: { component: timeline }
  cards: { component: cards }
  meters: { component: meters }
  gallery: { component: gallery }
  kv: { component: kv }
  columns: { component: columns }
  hero: { component: hero }
  timeline-empty: { component: timeline }
  orphan-a: { component: timeline }
  cards-leading: { component: cards }
  broken: { component: cards, class: "not-valid!" }
  timeline-extra: { component: timeline }
  orphan-b: { component: hero }
  meters-over: { component: meters }
  unknown: { component: mystery }
  timeline-paragraph: { component: timeline }
  timeline-empty-item: { component: timeline }
  cards-empty: { component: cards }
  meters-paragraph: { component: meters }
  meters-extra: { component: meters }
  meters-cols: { component: meters }
  meters-norows: { component: meters }
  meters-negative: { component: meters }
  meters-nonfinite: { component: meters }
  meters-unit: { component: meters }
  gallery-paragraph: { component: gallery }
  gallery-empty: { component: gallery }
  kv-extra: { component: kv }
  kv-task: { component: kv }
  kv-ordered: { component: kv }
  kv-space: { component: kv }
  kv-empty: { component: kv }
  columns-one: { component: columns }
  columns-empty: { component: columns }
  hero-two-images: { component: hero }
  hero-empty: { component: hero }
  kv-table: { component: kv }
  kv-table-cols: { component: kv }
  meters-missing: { component: meters }
---
# Timeline
- One
- Two

# Cards
## Alpha
A.

## Beta
B.

# Meters
| Label | Value |
| --- | --- |
| CPU | 80 |
| Disk | 0 |

# Gallery
![First](first.png)

![Second](second.png)

# Kv
- **Mode**: Safe
- **Owner**: Team

# Columns
Left.

Right.

# Hero
Text.

![Cover](cover.png)

# Timeline Empty

# Timeline Paragraph
Text.

# Timeline Extra
- One

Paragraph.

# Timeline Empty Item
- One
- 

# Cards Leading
Paragraph.

## Child

# Cards Empty

# Meters Paragraph
Text.

# Meters Extra
| Label | Value |
| --- | --- |
| CPU | 80 |

Paragraph.

# Meters Cols
| A | B | C |
| --- | --- | --- |
| 1 | 2 | 3 |

# Meters Norows
| Label | Value |
| --- | --- |

# Meters Negative
| Label | Value |
| --- | --- |
| CPU | -1 |

# Meters Over
| Label | Value |
| --- | --- |
| CPU | 101 |

# Meters Nonfinite
| Label | Value |
| --- | --- |
| CPU | NaN |

# Meters Unit
| Label | Value |
| --- | --- |
| CPU | 80% |

# Gallery Paragraph
![First](first.png)

Text.

# Gallery Empty

# Kv Extra
- **Mode**: Safe

Paragraph.

# Kv Task
- [x] **Mode**: Safe

# Kv Ordered
1. **Mode**: Safe

# Kv Space
- **Mode** : Safe

# Kv Empty

# Columns One
Only.

# Columns Empty

# Hero Two Images
![First](first.png)

![Second](second.png)

# Hero Empty

# Broken
Content.

# Unknown
Content.

# Kv Table
| Mode | Safe |
| --- | --- |
| Owner | Team |

# Kv Table Cols
| A | B | C |
| --- | --- | --- |
| 1 | 2 | 3 |

# Meters Missing
| Label | Value |
| --- | --- |
| CPU |
