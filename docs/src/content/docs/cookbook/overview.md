---
title: Cookbook
description: Complete, CI-checked rws configurations for common stacks — static sites, SPAs, Node, Python, Ruby and PHP apps, internal tools behind single sign-on, API gateways and local development.
---

Each recipe is one `rws.config.toml` you can copy, change the names in, and
run. Every TOML block on these pages is checked by `rws --check` in CI, so
the recipes keep working as rws changes.

| Recipe | For |
|---|---|
| [Static site](/cookbook/static-site/) | A site of files with automatic HTTPS |
| [Single-page app + API](/cookbook/single-page-app/) | React / Vue / Svelte front end with an API behind `/api` |
| [Node apps](/cookbook/node-apps/) | Next.js, Nuxt, Remix, SvelteKit, Express |
| [Python and Ruby apps](/cookbook/python-ruby/) | Django, FastAPI, Flask, Rails — app server, static files, WebSockets |
| [PHP apps](/cookbook/php/) | Laravel, Symfony, WordPress through php-fpm |
| [Internal tools behind sign-in](/cookbook/internal-tools/) | Grafana, Gitea, admin panels behind Google / Okta / Keycloak or Authelia |
| [API gateway](/cookbook/api-gateway/) | Several services behind one host: JWT, rate limits, CORS, caching |
| [Local development](/cookbook/local-development/) | `https://localhost` and a dev proxy |

Run a recipe with `rws --check` first; it reports unknown keys, missing
files and unset secrets before anything starts:

```sh
rws --check=rws.config.toml && rws
```
