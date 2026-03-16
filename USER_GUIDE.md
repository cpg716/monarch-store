# MonARCH Store User Guide

**Current frontend:** GTK  
**Last updated:** 2026-03-09

MonARCH Store is a software manager for Arch-based systems built around **Iron Core**, the Rust backend that discovers repositories, hydrates metadata, merges stable package variants into a canonical app identity, and supplies that data to the GTK interface.

GTK is the active MonARCH frontend. The older Tauri/React interface is legacy/reference-only.

## What MonARCH does

MonARCH gives you one place to work with:
- Arch and distro-native repositories
- Chaotic-AUR, when supported by the host
- Flatpak / Flathub
- AUR builds through the native MonARCH AUR engine

MonARCH does not try to replace your system's package policy. It follows the host's repository configuration, Arch safety rules, and the Iron Core package model.

## Core product model

MonARCH is designed around a single rule:
- **Iron Core decides what an app is**

That means:
- Search, Home, Categories, Installed, Updates, cards, and details all come from the same canonical backend identity
- Stable cross-source variants are merged into one app listing
- Channel variants such as beta, nightly, canary, dev, and ESR remain separate
- The GTK frontend renders what Iron Core sends; it does not parse or guess metadata on its own

## Main surfaces

### Home
Home is the discovery surface for:
- Featured apps
- Recommended essentials
- Trending apps
- Categories

These sections are backend-fed. If a source is hidden in Settings, it should disappear from discovery surfaces but still remain valid for Installed and Updates when the user already has packages from that source.

### Search
Search is the primary place to find apps across all enabled discovery sources.

Search behavior:
- One canonical listing per stable app
- Source-aware ranking with Arch-native sources first
- Optional source filtering
- Optional category filtering
- Optional `Show system apps`

### Details
Details is the single action surface for an app.

This is where the user sees:
- install / uninstall / open / update state
- source selection
- source-specific facts
- screenshots
- long description
- links
- review and metadata surfaces

Cards are intentionally lightweight. The details page is where source choice and package state belong.

### Library
Library shows installed applications from all supported sources, including sources that may be hidden from discovery.

### Updates
Updates shows available package updates across enabled system sources and already-installed apps from supported sources.

## Source toggles

Settings can limit which sources appear in **Search** and **Home/Discovery**:
- Flatpak
- Chaotic-AUR
- AUR
- System apps visibility

Important behavior:
- Turning a source off in discovery does **not** remove already-installed apps from Library or Updates
- MonARCH still needs to surface installed apps and pending updates accurately

## System apps toggle

`Show system apps` controls whether lower-level or non-user-facing packages appear in discovery and search.

Expected behavior:
- Off: discovery/search focus on user-facing software
- On: system-facing packages are also visible in search/discovery
- Installed and updates remain truthful regardless of this toggle

## Source selection

When an app is available from multiple stable sources, MonARCH presents one listing and lets the user inspect and choose among the available sources in Details.

Expected behavior:
- The details page reloads the visible package facts when a different source is selected
- Installed-source rules are respected
- If a package is already installed from one source, MonARCH should not pretend that it is installed from another source

## Metadata and logos

MonARCH aims to show every app as a professional listing:
- real app icon when available from AppStream, Flathub, repo metadata, or merged backend hydration
- screenshots and long descriptions when available
- source badges ordered by importance
- intentional fallback presentation when metadata is incomplete

The GTK frontend should not invent metadata. It should render the richest trusted data that Iron Core provides.

## Package safety rules

MonARCH follows Arch-safe behavior:
- no partial-upgrade workflow
- repo package installs are update-aware
- helper performs ALPM writes
- AUR builds run unprivileged, then install through the helper

## GTK and legacy Tauri

GTK is the current product.

Tauri/React remains in the repository only as:
- historical/reference implementation material
- parity reference while GTK closes remaining gaps

Do not treat Tauri behavior as the current shipped UX unless a document explicitly marks it as historical comparison material.
