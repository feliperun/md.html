---
title: Document scan fixture
subtitle: synthetic evidence for T11a
---

# Heading 1 {#custom-h1}

A paragraph with *single emphasis* and _another emphasis_ but no snake_case_identifier, no **strong**, no __strong__, no `*code*`.

Here is an image: ![Alt text](images/diagram.png "Diagram Title") and HTML image <img src="images/photo.jpg" alt="Photo">.
Angle destination ![Angle](<images/nested path/pic.png>), escaped ![Esc](img/esc\(1\).png), remote ![Remote](https://cdn.example.com/logo.png), data ![Data](data:image/png;base64,iVBORw0KGgo=).
Reference image ![Ref Image][REF-1], collapsed ![Collapsed][], shortcut ![Shortcut].

[REF-1]: images/ref-target.png "Ref Title"
[collapsed]: images/collapsed.png
[shortcut]: images/shortcut.png

<!-- HTML comment with ![ignored image](ignored.png), *ignored emphasis*, and # Ignored Heading -->

```rust
// In fence: ![not.png](not.png), *not emphasis*, # Not Heading, ::: not-a-container
```

    Indented code: ![not2.png](not2.png), *not emphasis*, # Not Heading

::: note | Optional Note Note
Inside note callout.
:::

:::: {.warning}
Nested warning.
::: details | Summary Text
Hidden content.
:::
::::

::: quote |
Empty argument stays null.
:::

::: note trailing garbage stays prose
:::

## Heading with closing hashes ###

### Heading with inline *source* and `code` {#h3-id}
