# Surface website

A small static site that hosts the Surface language documentation,
built with [Genereto](https://gh.fponzi.me/genereto/).

```
website/
  config.yml                      Genereto site config
  content/
    index.md                      Landing page (hero + cards)
    blog/
      overview.md   → ../docs/overview.md     (symlink)
      language-spec.md  → …                   (symlink)
      modules.md, coverage.md, changelog.md   (symlinks)
  templates/
    surface/
      index.html                  Landing template (full-width)
      blog.html                   Doc-page template (sidebar + article)
      header.html / footer.html / sidebar.html   Shared partials
      res/styles.css              Blue-themed stylesheet
  build.sh                        Wraps genereto + rewrites .md→.html links
```

The doc pages are **symlinks** back to `../docs/*.md`. That keeps the
markdown source canonical in `docs/` and means the site never drifts.
Each `docs/*.md` carries a small YAML frontmatter block at the top
(title, description, ToC flag) that Genereto consumes.

## Build locally

You need the `genereto` binary (Rust SSG, ~4 MB). Either:

**Pre-built (Linux x86_64, glibc ≥ 2.39 needed):**

```bash
curl -L -o /tmp/genereto.tar.gz \
  https://github.com/FedericoPonzi/genereto/releases/latest/download/genereto-v0.4.4-linux-x86_64.tar.gz
tar xzf /tmp/genereto.tar.gz -C /tmp
sudo install /tmp/genereto /usr/local/bin/
```

**From source (any platform with Rust):**

```bash
git clone https://github.com/FedericoPonzi/genereto.git
cd genereto && cargo build --release
# binary at target/release/genereto
```

Then build the site:

```bash
./build.sh                           # uses `genereto` from $PATH
./build.sh --bin=/path/to/genereto   # or point at a specific binary
```

The output lands at `website/output/`. Open `output/index.html` in a
browser or serve it:

```bash
python3 -m http.server -d output 8000
# → http://localhost:8000/
```

## Editing content

- **Landing page**: edit `content/index.md`. Frontmatter and embedded
  HTML / markdown are intentional — the cards + hero are styled by
  classes in `res/styles.css`.
- **Doc pages**: edit the canonical files in `../docs/*.md`. Symlinks
  pick up the change automatically.
- **Styling**: edit `templates/surface/res/styles.css`. The blue
  palette lives in CSS variables at the top.

## Deploy

A GitHub Actions workflow at `.github/workflows/deploy-site.yml`
builds and publishes the site to GitHub Pages on every push to `main`
that touches `website/**` or `docs/**`. It uses Genereto's official
build action (`FedericoPonzi/genereto/.github/actions/build-site@v0.1.0-ga`).
Set `url:` in `config.yml` to the GitHub Pages URL of your repo
before merging.

## Notes

- The post-build pass in `build.sh` rewrites `.md` links to `.html`
  so cross-page nav works on the web. Doc-internal links like
  `../TODO.md` and `../examples/` are rewritten to GitHub blob URLs
  (override the base via `SURFACE_REPO_URL`).
- Genereto generates a stray `blog-index.html` because it always
  emits one for blog dirs. We don't link to it; harmless.
