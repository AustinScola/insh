// Adds a version picker to the menu bar and, when the version being read is not the newest one, a
// banner above the content.
//
// The versions are fetched from `versions.json` at the root of the site every time a page loads
// rather than being baked in at build time, so that already published versions list newly released
// ones without having to be rebuilt.
//
// When there is no `versions.json` (which is the case when a single book is built or served on its
// own), nothing is added.

(function () {
    if (typeof path_to_root === "undefined") {
        return;
    }

    // `path_to_root` is set by mdbook and is relative to the page. It is empty for the pages at the
    // root of the book, and an empty relative URL resolves to the page itself rather than to the
    // directory the page is in.
    const bookRoot = new URL(path_to_root || "./", window.location.href);

    // The book is served from a directory of the site named after its version, so the root of the
    // site is one directory above the root of the book.
    const siteRoot = new URL("../", bookRoot);

    // The name of the directory the book is in.
    const version = decodeURIComponent(
        bookRoot.pathname.replace(/\/$/, "").split("/").pop()
    );

    function label(version, latest) {
        if (version === latest) {
            return version + " (latest)";
        }
        return version;
    }

    // Go to the same page of another version of the book if that version has it and to the start of
    // that version of the book otherwise.
    async function goTo(otherVersion) {
        const page = window.location.href.slice(bookRoot.href.length);
        const url = new URL(otherVersion + "/" + page, siteRoot);

        try {
            const response = await fetch(url, { method: "HEAD" });
            if (response.ok) {
                window.location.href = url.href;
                return;
            }
        } catch (error) {
            // Fall through to the start of the book.
        }

        window.location.href = new URL(otherVersion + "/", siteRoot).href;
    }

    function addPicker(versions, latest) {
        const menuBar = document.querySelector("#mdbook-menu-bar .right-buttons");
        if (menuBar === null) {
            return;
        }

        const picker = document.createElement("select");
        picker.id = "version-picker";
        picker.title = "Change version";
        picker.setAttribute("aria-label", "Change version");

        for (const otherVersion of versions) {
            const option = document.createElement("option");
            option.value = otherVersion;
            option.textContent = label(otherVersion, latest);
            option.selected = otherVersion === version;
            picker.appendChild(option);
        }

        picker.addEventListener("change", function () {
            goTo(picker.value);
        });

        menuBar.insertBefore(picker, menuBar.firstChild);
    }

    function addBanner(latest) {
        const content = document.querySelector("#mdbook-content");
        if (content === null) {
            return;
        }

        const banner = document.createElement("div");
        banner.id = "version-banner";

        const text = document.createElement("span");
        if (version === "master") {
            text.textContent =
                "This is the documentation for the unreleased development version of Insh.";
        } else {
            text.textContent =
                "This is the documentation for Insh " + version + ", which is not the latest release.";
        }
        banner.appendChild(text);

        const link = document.createElement("a");
        link.href = "#";
        link.textContent = "Go to the latest version.";
        link.addEventListener("click", function (event) {
            event.preventDefault();
            goTo(latest);
        });
        banner.appendChild(link);

        content.insertBefore(banner, content.firstChild);
    }

    async function addVersions() {
        let versions;
        let latest;
        try {
            const response = await fetch(new URL("versions.json", siteRoot));
            if (!response.ok) {
                return;
            }
            ({ versions, latest } = await response.json());
        } catch (error) {
            return;
        }

        if (!Array.isArray(versions) || !versions.includes(version)) {
            return;
        }

        addPicker(versions, latest);

        if (latest !== null && version !== latest) {
            addBanner(latest);
        }
    }

    addVersions();
})();
