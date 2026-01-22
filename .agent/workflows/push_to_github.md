---
description: How to push the current project to a new GitHub repository
---

This workflow will guide you through initializing a Git repository and pushing it to GitHub.

1.  **Initialize Git Repository**
    ```sh
    git init
    git branch -M main
    ```

2.  **Stage and Commit Files**
    ```sh
    git add .
    git commit -m "Initial commit: MonArch Store v0.1.0"
    ```

3.  **Create Repository on GitHub**
    *   Go to [https://github.com/new](https://github.com/new).
    *   Repo Name: `MonArch-Store` (or whatever you prefer).
    *   Description: "A modern software store for Arch Linux based on Tauri."
    *   **Do NOT** initialize with README, .gitignore, or License (we have them).
    *   Click **Create repository**.

4.  **Link and Push**
    *   Copy the URL provided by GitHub (e.g., `https://github.com/YOUR_USERNAME/MonArch-Store.git`).
    *   Run the command below (replace URL with yours):
    ```sh
    git remote add origin https://github.com/YOUR_USERNAME/MonArch-Store.git
    git push -u origin main
    ```
