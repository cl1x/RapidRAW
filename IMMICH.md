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
  and a stack with an image in an album does not count as "in no album".
- **Edit the original.** Opening an image downloads its original into a cache;
  until then it is shown like a file that is still in the cloud. For an image
  stacked with a RAW – an export on top of its original – the RAW is opened
  (can be switched off).
- **Edits are stored in Immich.** The `.rrdata` sidecar is saved as asset
  metadata on the server (key `rapidraw`), so an image looks the same on every
  computer. Local changes are sent a few seconds after they are made; newer
  edits from elsewhere are fetched when an image is opened. Can be switched
  off to keep edits local.
- **Exports go back to Immich.** An export is uploaded and stacked on top of
  its original, and put into the album the image was opened from (can be
  switched off). A previous export of the same image is replaced in its albums
  and stays in the stack; nothing is deleted.
- **Culling and sorting.** *Delete* moves an Immich image – and the image
  listed for it – to Immich's trash; *delete with associated files* takes the
  whole stack. Dragging images onto an Immich album adds them; dragging local
  files there uploads a copy, with its edits. Dropping on *In no album* only
  uploads.

Nothing changes until Immich is set up: the section stays hidden.

## Setup

1. In Immich, create an API key under *Account Settings → API Keys*. It needs
   access to albums, assets, stacks and asset metadata, and permission to
   upload.
2. In RapidRAW, open *Settings → Immich*, enter the server address and the key,
   test the connection and save.

The key is kept in the system's credential store (Secret Service, macOS
Keychain, Windows Credential Manager). Without one it goes into an owner-only
file in the app data folder, and the settings say so. On KDE, turn on the
Secret Service interface in the KDE Wallet settings.

Originals are cached in the app cache folder, 20 GB by default; the oldest
downloads are removed first, never ones with edits that have not reached the
server yet.

## How it fits into RapidRAW

The integration is a module of its own – `src-tauri/src/immich/` and
`src/components/immich/` – that the rest of the app calls in a few places:

| Where | What |
|---|---|
| `file_management::is_cloud_placeholder` | Immich images that are not downloaded yet count as cloud placeholders. |
| thumbnail generation | Placeholders get their thumbnail from Immich. |
| `load_image`, `export_images` | Download the original and fetch its latest edits first. |
| after each exported image | Upload the export if its source came from Immich. |
| `add_to_album`, deleting | Immich albums and Immich images are handed to the module. |
| folder tree, settings, album navigation | The sidebar section, the settings page, and loading Immich listings through the existing album flow. |

Each of these is a single call that does nothing for paths and albums that do
not belong to Immich. About 100 lines outside the module change in total.

Limits: listings show the newest 2000 images by default – adjustable in the
settings; the timeline and the filter reach the others – and when the same image is edited on two computers at once,
the local edit wins. Tested on Linux against Immich 3.2 only.

## Upstream

The goal is to contribute this to RapidRAW – possibly in a more general form,
as an interface for remote libraries with Immich as its first implementation.
Until then this fork follows upstream closely. The code without this page and
the notice in the README is on the branch
[`feature/immich`](../../tree/feature/immich).

## AI disclosure

This integration was developed with **Claude (Anthropic), an AI coding
assistant**. I (cl1x) defined the requirements, made the design decisions and
tested it with my own library; Claude wrote most of the code. Commits are
marked with `Co-Authored-By: Claude`. Please review the code with this in mind.
