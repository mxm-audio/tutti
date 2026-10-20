# Extensions (tutti 1.0, draft)

How a program keeps its own data in a project without breaking anyone else's, and how such data
becomes part of the format.

## 1. Where extension data lives

Every object that has an `extensions` key (the document, a track, the master, a device, a
pattern, a part, an event, an audio piece, an automation lane) may carry vendor data under it:

```json
"extensions": {
  "dk.mxm.newdawn": { "armed": true }
}
```

- A **namespace** is a reverse-DNS name, lower case, matching `^[a-z0-9]+(\.[a-z0-9-]+)+$`: a
  domain you control, then whatever you like. CLAP names its plugins and extensions the same way.
- The value under a namespace is any JSON value; its shape is the namespace owner's business.
- There is no free-form `extras` key: everything that is not core is namespaced, so a reader can
  always tell vendor data from a future core key.

## 2. Used and required

The document lists every namespace that appears anywhere in it under `extensions_used`, and
under `extensions_required` the subset without which the project **does not sound right**.

- A required extension's data MUST leave a **core fallback**: the object it extends plays
  acceptably through the core fields alone. (A vendor's curve type on an automation point must
  still carry `hold` or `linear`; a vendor's stretch algorithm still carries the map and the bpm;
  a vendor's device must still carry a render or a snapshot.) This is glTF's rule, and it is what
  lets a reader from 2026 open a project from 2032.
- A reader lacking a required extension opens the project anyway, says by name what it cannot
  honour, plays the fallbacks and preserves everything (`tutti.md` 8.1). It is never a reason to
  refuse.
- Anything else a program keeps (its view state, its undo history's hints, its user's colours) is
  *used*, not required.

## 3. Preservation

A reader keeps every `extensions` value it reads, as the same JSON value on the same object, and
a writer writes it back (`tutti.md` 8.2). A program that deletes an object deletes the extension
data on it. A program never edits another program's namespace.

## 4. The registry

Known namespaces, so that two programs never guess at each other's data. Registration is a pull
request adding a row; an unregistered namespace is valid and simply opaque to everyone else.

| Namespace | Owner | What it holds | Since |
|---|---|---|---|
| `dk.mxm.newdawn` | newDAWn ([maxmcorp/newDAWn](https://github.com/maxmcorp/newDAWn)) | On a track: `armed`. On the document: the save `generation` its journal extends, and view state. Never required. | 1.0 |

Reserved prefixes: `org.tutti.*` for drafts of core features (section 5); nothing else is
reserved.

## 5. From an extension to the core

An idea proves itself as an extension first, in its owner's namespace or in `org.tutti.draft.*`.
When it is adopted into the core:

1. The next **minor** version adds the core keys with their defaults and fallbacks.
2. A writer at that version writes the core form, and MAY also write the extension form for one
   major version, for readers that have not moved.
3. A reader at that version prefers the core form when both are present.
4. The registry row gains `superseded_by: core <version>`; the namespace stays valid and
   preserved for ever.

Nothing is ever removed from the core within a major version (`tutti.md` 8.3).
