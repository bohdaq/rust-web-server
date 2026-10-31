import { LitElement, css, html } from "lit";

// Material icon "delete" (Apache 2.0).
const deleteIcon = html`<svg viewBox="0 0 24 24" aria-hidden="true">
  <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"></path>
</svg>`;

/** notes page: lists, adds and deletes notes through /api/notes. */
export class NotesApp extends LitElement {
  static properties = {
    notes: { state: true },
    loaded: { state: true },
    draft: { state: true },
  };

  static styles = css`
    :host {
      display: block;
      max-width: 40rem;
      margin: 2rem auto;
      padding: 0 1rem;
      font-family: var(--md-sys-typescale-body-large-font, system-ui, sans-serif);
      color: var(--md-sys-color-on-surface, #1d1b20);
    }
    h1 {
      font-family: var(--md-sys-typescale-headline-medium-font, inherit);
      font-size: var(--md-sys-typescale-headline-medium-size, 1.75rem);
      font-weight: var(--md-sys-typescale-headline-medium-weight, 400);
    }
    form {
      display: flex;
      align-items: flex-start;
      gap: 0.5rem;
      margin-bottom: 1rem;
    }
    lit-material-text-field {
      flex: 1;
      --lit-material-text-field-min-width: 0;
    }
    lit-material-button {
      margin-top: 0.5rem;
    }
    svg {
      width: 100%;
      height: 100%;
      fill: currentColor;
    }
  `;

  constructor() {
    super();
    this.notes = [];
    this.loaded = false;
    this.draft = "";
  }

  connectedCallback() {
    super.connectedCallback();
    this.load();
  }

  async request(method, path, body) {
    const response = await fetch(path, {
      method,
      headers: body ? { "content-type": "application/json" } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error || response.statusText);
    return data;
  }

  /** Shows a failed request in the snackbar. */
  report(error) {
    this.renderRoot.querySelector("lit-material-snackbar")?.show(error.message);
  }

  async load() {
    try {
      this.notes = await this.request("GET", "/api/notes");
    } catch (e) {
      this.report(e);
    } finally {
      this.loaded = true;
    }
  }

  async add(event) {
    event.preventDefault();
    const text = this.draft.trim();
    if (!text) return;
    try {
      await this.request("POST", "/api/notes", { text });
      this.draft = "";
      await this.load();
    } catch (e) {
      this.report(e);
    }
  }

  async remove(id) {
    try {
      await this.request("DELETE", `/api/notes/${id}`);
      await this.load();
    } catch (e) {
      this.report(e);
    }
  }

  render() {
    return html`
      <h1>Notes</h1>
      <form @submit=${this.add}>
        <lit-material-text-field
          variant="outlined"
          label="New note"
          autocomplete="off"
          .value=${this.draft}
          @input=${(e) => (this.draft = e.target.value)}
        ></lit-material-text-field>
        <lit-material-button type="submit" ?disabled=${!this.draft.trim()}>Add</lit-material-button>
      </form>
      ${this.notes.length
        ? html`<lit-material-list>
            ${this.notes.map(
              (note) => html`<lit-material-list-item divider>
                ${note.text}
                <lit-material-icon-button
                  slot="trailing"
                  aria-label=${`Delete note: ${note.text}`}
                  @click=${() => this.remove(note.id)}
                  ><span slot="icon">${deleteIcon}</span></lit-material-icon-button
                >
              </lit-material-list-item>`,
            )}
          </lit-material-list>`
        : this.loaded
          ? html`<lit-material-empty-state size="compact" headline="No notes yet">
              Write one above and press Add.
            </lit-material-empty-state>`
          : ""}
      <lit-material-snackbar closable></lit-material-snackbar>
    `;
  }
}

customElements.define("notes-app", NotesApp);
