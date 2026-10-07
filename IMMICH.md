# Immich integration

This fork of RapidRAW can work directly on the photos of an
[Immich](https://immich.app) server. Immich stays the only place your photos
and their edits live; RapidRAW only keeps a cache.

> **Unofficial.** This is not (yet) part of RapidRAW and not affiliated with
> Immich. The goal is to contribute it upstream – see [Upstream](#upstream).
> Keep backups of your photos, as with any new software.

## What it does

- **Browse Immich in the library.** An *Immich* section in the sidebar shows
  your albums, a timeline by year and month, the photos that are in no album,
  and a filter (album, date range, country and city, camera, person,
  favourites). Thumbnails come from Immich right away. A stack appears once,
  and a stack with an image in an album does not count as "in no album". The
  albums follow the sort chosen for the folders in the sidebar menu.
- **Edit the original.** Opening an image downloads its original into a cache;
  until then it is shown like a file that is still in the cloud. For an image
  stacked with a RAW – an export on top of its original – the RAW is opened.
- **Edits are stored in Immich.** The `.rrdata` sidecar is saved as asset
  metadata on the server (key `rapidraw`), so an image looks the same on every
  computer. Newer edits from elsewhere are fetched when an image is opened;
  local changes are sent a few seconds after they are made. Edits made offline
  are sent once the server is reachable again, also after a restart. If an
  image was edited both here and on another computer, nothing is overwritten:
  the local edits stay and the other version is kept as a virtual copy. The
  sidebar shows when the server is offline and how many edits are waiting.
- **Export to Immich.** *Immich* is a destination in the export panel, next
  to the folders. The export is uploaded, stacked on top of its original and
  put into the album of your choice: the one the image was opened from,
  another one or none. Images from a local folder can bring their original
  along: it is uploaded with its edits, and the export is stacked on top.
  Previous exports are kept. Not available on Android, where exports go to
  the system's downloads.
- **Culling and sorting.** *Delete* moves an Immich image – and the image
  listed for it – to Immich's trash; *delete with associated files* takes the
  whole stack. *Remove from album* removes images from an Immich album.
  Dragging images onto an Immich album adds them; dragging local files there
  uploads a copy, with its edits. Dropping on *In no album* only uploads.

Nothing changes until Immich is set up: the section stays hidden, and nothing
is read from the keyring.

## Setup

Immich 2.5 or newer is needed.

1. In Immich, create an API key under *Account Settings → API Keys* with these
   permissions: `album.read`, `albumAsset.create`, `albumAsset.delete`,
   `asset.read`, `asset.view`, `asset.download`, `asset.upload`,
   `asset.update`, `asset.delete`, `stack.read`, `stack.create`, `user.read`,
   and `person.read` for the person filter.
2. In RapidRAW, open *Settings → Immich*, enter the server address and the key,
   test the connection and save.

The options are part of RapidRAW's settings. The key is kept apart from them
in the system's credential store (Secret Service, macOS Keychain, Windows
Credential Manager, Android Keystore). Without one it goes into an owner-only
file in the app data folder, and the settings say so. On KDE, turn on the
Secret Service interface in the KDE Wallet settings.

Originals are cached in the app cache folder, separately for each server,
20 GB by default. The limit can be changed and the cache cleared on the
storage page of the settings. The least recently opened originals are removed
first, edits never.

## How it fits into RapidRAW

The integration is a module of its own – `src-tauri/src/immich/` and
`src/components/immich/` – that the rest of the app calls in a few places:

| Where | What |
|---|---|
| `file_management::is_cloud_placeholder` | Immich images that are not downloaded yet count as cloud placeholders. |
| thumbnail generation | Placeholders get their thumbnail from Immich. |
| `load_metadata`, `load_image`, `export_images` | Fetch the latest edits and download the original first. |
| after each exported image | Upload the export if *Immich* was chosen as the destination. |
| `add_to_album`, deleting, *Remove from album* | Immich albums and Immich images are handed to the module. |
| app start | Edits that have not reached the server yet are sent, if Immich is set up. |
| folder tree, settings, album navigation, export panel | The sidebar section, the settings page, loading Immich listings through the existing album flow, and the export destination. |

Each of these is a single call that does nothing for paths and albums that do
not belong to Immich.

Large albums and months are loaded in full, several pages at a time. Tested
on Linux and Android against Immich 3.3.

## Upstream

Most of this is part of RapidRAW since
[CyberTimon/RapidRAW#1879](https://github.com/CyberTimon/RapidRAW/pull/1879):
browsing, editing, keeping edits in Immich, export to Immich and deleting.
Dragging images onto albums, the *In no album* view, the filter and the album
order are meant to follow as separate pull requests. The code without this
page and the notice in the README is on the branch
[`feature/immich`](../../tree/feature/immich).

## AI disclosure

This integration was developed with **Claude (Anthropic), an AI coding
assistant**. I (cl1x) defined the requirements, made the design decisions and
tested it with my own library; Claude wrote most of the code. Commits are
marked with `Co-Authored-By: Claude`. Please review the code with this in mind.
