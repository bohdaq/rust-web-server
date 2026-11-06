<!-- DRAFT for review (spec/SEO_GEO.md): a guide for lit.rws8.tech
(apps/site/src/pages/guides.js, "Start" group). Lines marked [verify] state
facts about other libraries — check each against their current docs. -->

# lit-material vs Material Web, Shoelace and Vaadin

**Short answer:** lit-material is for **consoles and admin UIs**: Material 3
components *plus* the parts those apps need that Material Web leaves out —
data tables, log viewers, charts, navigation shells, forms — each its own npm
package, all rendering on the server. Use **Material Web** for plain Material 3
controls, **Shoelace/Web Awesome** for a non-Material design system, and
**Vaadin** if you want a Java-backed framework.

| | lit-material | Material Web (`@material/web`) | Shoelace / Web Awesome | Vaadin |
|---|---|---|---|---|
| Design | Material 3 | Material 3 | Its own [verify] | Lumo / Material [verify] |
| Admin parts (table, log viewer, charts, nav shell) | Yes | No — buttons, fields, lists, dialogs [verify] | Partly [verify] | Yes (grid, charts commercial) [verify] |
| Server rendering (Lit SSR) | Every component | Partly [verify] | No [verify] | Server-driven, different model [verify] |
| Packaging | One npm package per component | One package [verify] | One package [verify] | Framework [verify] |
| Status | Active | Maintenance mode since 2024 [verify] | Active as Web Awesome [verify] | Active |
| License | MIT | Apache-2.0 [verify] | MIT [verify] | Apache-2.0 + commercial [verify] |

## When to choose which

- **lit-material**: a console, dashboard or admin UI that should look Material 3,
  render on the server and ship only the components it uses.
- **Material Web**: a few Material 3 controls in an existing app.
- **Shoelace / Web Awesome**: a framework-agnostic design system that isn't Material.
- **Vaadin**: a Java team building business apps end to end.

Next: [Install lit-material](/guides/install/) · [Server rendering](/guides/server-rendering/) · [Migrating from Material Web](/guides/migrate-material-web/)
