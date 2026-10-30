var e=globalThis,t=e.ShadowRoot&&(e.ShadyCSS===void 0||e.ShadyCSS.nativeShadow)&&`adoptedStyleSheets`in Document.prototype&&`replace`in CSSStyleSheet.prototype,n=Symbol(),r=new WeakMap,i=class{constructor(e,t,r){if(this._$cssResult$=!0,r!==n)throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");this.cssText=e,this.t=t}get styleSheet(){let e=this.o,n=this.t;if(t&&e===void 0){let t=n!==void 0&&n.length===1;t&&(e=r.get(n)),e===void 0&&((this.o=e=new CSSStyleSheet).replaceSync(this.cssText),t&&r.set(n,e))}return e}toString(){return this.cssText}},a=e=>new i(typeof e==`string`?e:e+``,void 0,n),o=(e,...t)=>new i(e.length===1?e[0]:t.reduce((t,n,r)=>t+(e=>{if(!0===e._$cssResult$)return e.cssText;if(typeof e==`number`)return e;throw Error(`Value passed to 'css' function must be a 'css' function result: `+e+`. Use 'unsafeCSS' to pass non-literal values, but take care to ensure page security.`)})(n)+e[r+1],e[0]),e,n),s=(n,r)=>{if(t)n.adoptedStyleSheets=r.map(e=>e instanceof CSSStyleSheet?e:e.styleSheet);else for(let t of r){let r=document.createElement(`style`),i=e.litNonce;i!==void 0&&r.setAttribute(`nonce`,i),r.textContent=t.cssText,n.appendChild(r)}},c=t?e=>e:e=>e instanceof CSSStyleSheet?(e=>{let t=``;for(let n of e.cssRules)t+=n.cssText;return a(t)})(e):e,{is:l,defineProperty:u,getOwnPropertyDescriptor:d,getOwnPropertyNames:ee,getOwnPropertySymbols:te,getPrototypeOf:ne}=Object,re=globalThis,ie=re.trustedTypes,ae=ie?ie.emptyScript:``,oe=re.reactiveElementPolyfillSupport,f=(e,t)=>e,se={toAttribute(e,t){switch(t){case Boolean:e=e?ae:null;break;case Object:case Array:e=e==null?e:JSON.stringify(e)}return e},fromAttribute(e,t){let n=e;switch(t){case Boolean:n=e!==null;break;case Number:n=e===null?null:Number(e);break;case Object:case Array:try{n=JSON.parse(e)}catch{n=null}}return n}},ce=(e,t)=>!l(e,t),le={attribute:!0,type:String,converter:se,reflect:!1,useDefault:!1,hasChanged:ce};Symbol.metadata??=Symbol(`metadata`),re.litPropertyMetadata??=new WeakMap;var p=class extends HTMLElement{static addInitializer(e){this._$Ei(),(this.l??=[]).push(e)}static get observedAttributes(){return this.finalize(),this._$Eh&&[...this._$Eh.keys()]}static createProperty(e,t=le){if(t.state&&(t.attribute=!1),this._$Ei(),this.prototype.hasOwnProperty(e)&&((t=Object.create(t)).wrapped=!0),this.elementProperties.set(e,t),!t.noAccessor){let n=Symbol(),r=this.getPropertyDescriptor(e,n,t);r!==void 0&&u(this.prototype,e,r)}}static getPropertyDescriptor(e,t,n){let{get:r,set:i}=d(this.prototype,e)??{get(){return this[t]},set(e){this[t]=e}};return{get:r,set(t){let a=r?.call(this);i?.call(this,t),this.requestUpdate(e,a,n)},configurable:!0,enumerable:!0}}static getPropertyOptions(e){return this.elementProperties.get(e)??le}static _$Ei(){if(this.hasOwnProperty(f(`elementProperties`)))return;let e=ne(this);e.finalize(),e.l!==void 0&&(this.l=[...e.l]),this.elementProperties=new Map(e.elementProperties)}static finalize(){if(this.hasOwnProperty(f(`finalized`)))return;if(this.finalized=!0,this._$Ei(),this.hasOwnProperty(f(`properties`))){let e=this.properties,t=[...ee(e),...te(e)];for(let n of t)this.createProperty(n,e[n])}let e=this[Symbol.metadata];if(e!==null){let t=litPropertyMetadata.get(e);if(t!==void 0)for(let[e,n]of t)this.elementProperties.set(e,n)}this._$Eh=new Map;for(let[e,t]of this.elementProperties){let n=this._$Eu(e,t);n!==void 0&&this._$Eh.set(n,e)}this.elementStyles=this.finalizeStyles(this.styles)}static finalizeStyles(e){let t=[];if(Array.isArray(e)){let n=new Set(e.flat(1/0).reverse());for(let e of n)t.unshift(c(e))}else e!==void 0&&t.push(c(e));return t}static _$Eu(e,t){let n=t.attribute;return!1===n?void 0:typeof n==`string`?n:typeof e==`string`?e.toLowerCase():void 0}constructor(){super(),this._$Ep=void 0,this.isUpdatePending=!1,this.hasUpdated=!1,this._$Em=null,this._$Ev()}_$Ev(){this._$ES=new Promise(e=>this.enableUpdating=e),this._$AL=new Map,this._$E_(),this.requestUpdate(),this.constructor.l?.forEach(e=>e(this))}addController(e){(this._$EO??=new Set).add(e),this.renderRoot!==void 0&&this.isConnected&&e.hostConnected?.()}removeController(e){this._$EO?.delete(e)}_$E_(){let e=new Map,t=this.constructor.elementProperties;for(let n of t.keys())this.hasOwnProperty(n)&&(e.set(n,this[n]),delete this[n]);e.size>0&&(this._$Ep=e)}createRenderRoot(){let e=this.shadowRoot??this.attachShadow(this.constructor.shadowRootOptions);return s(e,this.constructor.elementStyles),e}connectedCallback(){this.renderRoot??=this.createRenderRoot(),this.enableUpdating(!0),this._$EO?.forEach(e=>e.hostConnected?.())}enableUpdating(e){}disconnectedCallback(){this._$EO?.forEach(e=>e.hostDisconnected?.())}attributeChangedCallback(e,t,n){this._$AK(e,n)}_$ET(e,t){let n=this.constructor.elementProperties.get(e),r=this.constructor._$Eu(e,n);if(r!==void 0&&!0===n.reflect){let i=(n.converter?.toAttribute===void 0?se:n.converter).toAttribute(t,n.type);this._$Em=e,i==null?this.removeAttribute(r):this.setAttribute(r,i),this._$Em=null}}_$AK(e,t){let n=this.constructor,r=n._$Eh.get(e);if(r!==void 0&&this._$Em!==r){let e=n.getPropertyOptions(r),i=typeof e.converter==`function`?{fromAttribute:e.converter}:e.converter?.fromAttribute===void 0?se:e.converter;this._$Em=r;let a=i.fromAttribute(t,e.type);this[r]=a??this._$Ej?.get(r)??a,this._$Em=null}}requestUpdate(e,t,n,r=!1,i){if(e!==void 0){let a=this.constructor;if(!1===r&&(i=this[e]),n??=a.getPropertyOptions(e),!((n.hasChanged??ce)(i,t)||n.useDefault&&n.reflect&&i===this._$Ej?.get(e)&&!this.hasAttribute(a._$Eu(e,n))))return;this.C(e,t,n)}!1===this.isUpdatePending&&(this._$ES=this._$EP())}C(e,t,{useDefault:n,reflect:r,wrapped:i},a){n&&!(this._$Ej??=new Map).has(e)&&(this._$Ej.set(e,a??t??this[e]),!0!==i||a!==void 0)||(this._$AL.has(e)||(this.hasUpdated||n||(t=void 0),this._$AL.set(e,t)),!0===r&&this._$Em!==e&&(this._$Eq??=new Set).add(e))}async _$EP(){this.isUpdatePending=!0;try{await this._$ES}catch(e){Promise.reject(e)}let e=this.scheduleUpdate();return e!=null&&await e,!this.isUpdatePending}scheduleUpdate(){return this.performUpdate()}performUpdate(){if(!this.isUpdatePending)return;if(!this.hasUpdated){if(this.renderRoot??=this.createRenderRoot(),this._$Ep){for(let[e,t]of this._$Ep)this[e]=t;this._$Ep=void 0}let e=this.constructor.elementProperties;if(e.size>0)for(let[t,n]of e){let{wrapped:e}=n,r=this[t];!0!==e||this._$AL.has(t)||r===void 0||this.C(t,void 0,n,r)}}let e=!1,t=this._$AL;try{e=this.shouldUpdate(t),e?(this.willUpdate(t),this._$EO?.forEach(e=>e.hostUpdate?.()),this.update(t)):this._$EM()}catch(t){throw e=!1,this._$EM(),t}e&&this._$AE(t)}willUpdate(e){}_$AE(e){this._$EO?.forEach(e=>e.hostUpdated?.()),this.hasUpdated||(this.hasUpdated=!0,this.firstUpdated(e)),this.updated(e)}_$EM(){this._$AL=new Map,this.isUpdatePending=!1}get updateComplete(){return this.getUpdateComplete()}getUpdateComplete(){return this._$ES}shouldUpdate(e){return!0}update(e){this._$Eq&&=this._$Eq.forEach(e=>this._$ET(e,this[e])),this._$EM()}updated(e){}firstUpdated(e){}};p.elementStyles=[],p.shadowRootOptions={mode:`open`},p[f(`elementProperties`)]=new Map,p[f(`finalized`)]=new Map,oe?.({ReactiveElement:p}),(re.reactiveElementVersions??=[]).push(`2.1.2`);var ue=globalThis,de=e=>e,fe=ue.trustedTypes,pe=fe?fe.createPolicy(`lit-html`,{createHTML:e=>e}):void 0,me=`$lit$`,m=`lit$${Math.random().toFixed(9).slice(2)}$`,he=`?`+m,ge=`<${he}>`,h=document,_e=()=>h.createComment(``),ve=e=>e===null||typeof e!=`object`&&typeof e!=`function`,ye=Array.isArray,be=e=>ye(e)||typeof e?.[Symbol.iterator]==`function`,xe=`[ 	
\f\r]`,Se=/<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g,Ce=/-->/g,we=/>/g,g=RegExp(`>|${xe}(?:([^\\s"'>=/]+)(${xe}*=${xe}*(?:[^ \t\n\f\r"'\`<>=]|("|')|))|$)`,`g`),Te=/'/g,Ee=/"/g,De=/^(?:script|style|textarea|title)$/i,Oe=e=>(t,...n)=>({_$litType$:e,strings:t,values:n}),_=Oe(1),v=Oe(2),y=Symbol.for(`lit-noChange`),b=Symbol.for(`lit-nothing`),ke=new WeakMap,x=h.createTreeWalker(h,129);function Ae(e,t){if(!ye(e)||!e.hasOwnProperty(`raw`))throw Error(`invalid template strings array`);return pe===void 0?t:pe.createHTML(t)}var je=(e,t)=>{let n=e.length-1,r=[],i,a=t===2?`<svg>`:t===3?`<math>`:``,o=Se;for(let t=0;t<n;t++){let n=e[t],s,c,l=-1,u=0;for(;u<n.length&&(o.lastIndex=u,c=o.exec(n),c!==null);)u=o.lastIndex,o===Se?c[1]===`!--`?o=Ce:c[1]===void 0?c[2]===void 0?c[3]!==void 0&&(o=g):(De.test(c[2])&&(i=RegExp(`</`+c[2],`g`)),o=g):o=we:o===g?c[0]===`>`?(o=i??Se,l=-1):c[1]===void 0?l=-2:(l=o.lastIndex-c[2].length,s=c[1],o=c[3]===void 0?g:c[3]===`"`?Ee:Te):o===Ee||o===Te?o=g:o===Ce||o===we?o=Se:(o=g,i=void 0);let d=o===g&&e[t+1].startsWith(`/>`)?` `:``;a+=o===Se?n+ge:l>=0?(r.push(s),n.slice(0,l)+me+n.slice(l)+m+d):n+m+(l===-2?t:d)}return[Ae(e,a+(e[n]||`<?>`)+(t===2?`</svg>`:t===3?`</math>`:``)),r]},Me=class e{constructor({strings:t,_$litType$:n},r){let i;this.parts=[];let a=0,o=0,s=t.length-1,c=this.parts,[l,u]=je(t,n);if(this.el=e.createElement(l,r),x.currentNode=this.el.content,n===2||n===3){let e=this.el.content.firstChild;e.replaceWith(...e.childNodes)}for(;(i=x.nextNode())!==null&&c.length<s;){if(i.nodeType===1){if(i.hasAttributes())for(let e of i.getAttributeNames())if(e.endsWith(me)){let t=u[o++],n=i.getAttribute(e).split(m),r=/([.?@])?(.*)/.exec(t);c.push({type:1,index:a,name:r[2],strings:n,ctor:r[1]===`.`?Ie:r[1]===`?`?Le:r[1]===`@`?Re:Fe}),i.removeAttribute(e)}else e.startsWith(m)&&(c.push({type:6,index:a}),i.removeAttribute(e));if(De.test(i.tagName)){let e=i.textContent.split(m),t=e.length-1;if(t>0){i.textContent=fe?fe.emptyScript:``;for(let n=0;n<t;n++)i.append(e[n],_e()),x.nextNode(),c.push({type:2,index:++a});i.append(e[t],_e())}}}else if(i.nodeType===8){if(i.data===he)c.push({type:2,index:a});else{let e=-1;for(;(e=i.data.indexOf(m,e+1))!==-1;)c.push({type:7,index:a}),e+=m.length-1}}a++}}static createElement(e,t){let n=h.createElement(`template`);return n.innerHTML=e,n}};function S(e,t,n=e,r){if(t===y)return t;let i=r===void 0?n._$Cl:n._$Co?.[r],a=ve(t)?void 0:t._$litDirective$;return i?.constructor!==a&&(i?._$AO?.(!1),a===void 0?i=void 0:(i=new a(e),i._$AT(e,n,r)),r===void 0?n._$Cl=i:(n._$Co??=[])[r]=i),i!==void 0&&(t=S(e,i._$AS(e,t.values),i,r)),t}var Ne=class{constructor(e,t){this._$AV=[],this._$AN=void 0,this._$AD=e,this._$AM=t}get parentNode(){return this._$AM.parentNode}get _$AU(){return this._$AM._$AU}u(e){let{el:{content:t},parts:n}=this._$AD,r=(e?.creationScope??h).importNode(t,!0);x.currentNode=r;let i=x.nextNode(),a=0,o=0,s=n[0];for(;s!==void 0;){if(a===s.index){let t;s.type===2?t=new Pe(i,i.nextSibling,this,e):s.type===1?t=new s.ctor(i,s.name,s.strings,this,e):s.type===6&&(t=new ze(i,this,e)),this._$AV.push(t),s=n[++o]}a!==s?.index&&(i=x.nextNode(),a++)}return x.currentNode=h,r}p(e){let t=0;for(let n of this._$AV)n!==void 0&&(n.strings===void 0?n._$AI(e[t]):(n._$AI(e,n,t),t+=n.strings.length-2)),t++}},Pe=class e{get _$AU(){return this._$AM?._$AU??this._$Cv}constructor(e,t,n,r){this.type=2,this._$AH=b,this._$AN=void 0,this._$AA=e,this._$AB=t,this._$AM=n,this.options=r,this._$Cv=r?.isConnected??!0}get parentNode(){let e=this._$AA.parentNode,t=this._$AM;return t!==void 0&&e?.nodeType===11&&(e=t.parentNode),e}get startNode(){return this._$AA}get endNode(){return this._$AB}_$AI(e,t=this){e=S(this,e,t),ve(e)?e===b||e==null||e===``?(this._$AH!==b&&this._$AR(),this._$AH=b):e!==this._$AH&&e!==y&&this._(e):e._$litType$===void 0?e.nodeType===void 0?be(e)?this.k(e):this._(e):this.T(e):this.$(e)}O(e){return this._$AA.parentNode.insertBefore(e,this._$AB)}T(e){this._$AH!==e&&(this._$AR(),this._$AH=this.O(e))}_(e){this._$AH!==b&&ve(this._$AH)?this._$AA.nextSibling.data=e:this.T(h.createTextNode(e)),this._$AH=e}$(e){let{values:t,_$litType$:n}=e,r=typeof n==`number`?this._$AC(e):(n.el===void 0&&(n.el=Me.createElement(Ae(n.h,n.h[0]),this.options)),n);if(this._$AH?._$AD===r)this._$AH.p(t);else{let e=new Ne(r,this),n=e.u(this.options);e.p(t),this.T(n),this._$AH=e}}_$AC(e){let t=ke.get(e.strings);return t===void 0&&ke.set(e.strings,t=new Me(e)),t}k(t){ye(this._$AH)||(this._$AH=[],this._$AR());let n=this._$AH,r,i=0;for(let a of t)i===n.length?n.push(r=new e(this.O(_e()),this.O(_e()),this,this.options)):r=n[i],r._$AI(a),i++;i<n.length&&(this._$AR(r&&r._$AB.nextSibling,i),n.length=i)}_$AR(e=this._$AA.nextSibling,t){for(this._$AP?.(!1,!0,t);e!==this._$AB;){let t=de(e).nextSibling;de(e).remove(),e=t}}setConnected(e){this._$AM===void 0&&(this._$Cv=e,this._$AP?.(e))}},Fe=class{get tagName(){return this.element.tagName}get _$AU(){return this._$AM._$AU}constructor(e,t,n,r,i){this.type=1,this._$AH=b,this._$AN=void 0,this.element=e,this.name=t,this._$AM=r,this.options=i,n.length>2||n[0]!==``||n[1]!==``?(this._$AH=Array(n.length-1).fill(new String),this.strings=n):this._$AH=b}_$AI(e,t=this,n,r){let i=this.strings,a=!1;if(i===void 0)e=S(this,e,t,0),a=!ve(e)||e!==this._$AH&&e!==y,a&&(this._$AH=e);else{let r=e,o,s;for(e=i[0],o=0;o<i.length-1;o++)s=S(this,r[n+o],t,o),s===y&&(s=this._$AH[o]),a||=!ve(s)||s!==this._$AH[o],s===b?e=b:e!==b&&(e+=(s??``)+i[o+1]),this._$AH[o]=s}a&&!r&&this.j(e)}j(e){e===b?this.element.removeAttribute(this.name):this.element.setAttribute(this.name,e??``)}},Ie=class extends Fe{constructor(){super(...arguments),this.type=3}j(e){this.element[this.name]=e===b?void 0:e}},Le=class extends Fe{constructor(){super(...arguments),this.type=4}j(e){this.element.toggleAttribute(this.name,!!e&&e!==b)}},Re=class extends Fe{constructor(e,t,n,r,i){super(e,t,n,r,i),this.type=5}_$AI(e,t=this){if((e=S(this,e,t,0)??b)===y)return;let n=this._$AH,r=e===b&&n!==b||e.capture!==n.capture||e.once!==n.once||e.passive!==n.passive,i=e!==b&&(n===b||r);r&&this.element.removeEventListener(this.name,this,n),i&&this.element.addEventListener(this.name,this,e),this._$AH=e}handleEvent(e){typeof this._$AH==`function`?this._$AH.call(this.options?.host??this.element,e):this._$AH.handleEvent(e)}},ze=class{constructor(e,t,n){this.element=e,this.type=6,this._$AN=void 0,this._$AM=t,this.options=n}get _$AU(){return this._$AM._$AU}_$AI(e){S(this,e)}},Be=ue.litHtmlPolyfillSupport;Be?.(Me,Pe),(ue.litHtmlVersions??=[]).push(`3.3.3`);var Ve=(e,t,n)=>{let r=n?.renderBefore??t,i=r._$litPart$;if(i===void 0){let e=n?.renderBefore??null;r._$litPart$=i=new Pe(t.insertBefore(_e(),e),e,void 0,n??{})}return i._$AI(e),i},He=globalThis,C=class extends p{constructor(){super(...arguments),this.renderOptions={host:this},this._$Do=void 0}createRenderRoot(){let e=super.createRenderRoot();return this.renderOptions.renderBefore??=e.firstChild,e}update(e){let t=this.render();this.hasUpdated||(this.renderOptions.isConnected=this.isConnected),super.update(e),this._$Do=Ve(t,this.renderRoot,this.renderOptions)}connectedCallback(){super.connectedCallback(),this._$Do?.setConnected(!0)}disconnectedCallback(){super.disconnectedCallback(),this._$Do?.setConnected(!1)}render(){return y}};C._$litElement$=!0,C.finalized=!0,He.litElementHydrateSupport?.({LitElement:C});var Ue=He.litElementPolyfillSupport;Ue?.({LitElement:C}),(He.litElementVersions??=[]).push(`4.2.2`);var w=e=>(t,n)=>{n===void 0?customElements.define(e,t):n.addInitializer(()=>{customElements.define(e,t)})},We={attribute:!0,type:String,converter:se,reflect:!1,hasChanged:ce},Ge=(e=We,t,n)=>{let{kind:r,metadata:i}=n,a=globalThis.litPropertyMetadata.get(i);if(a===void 0&&globalThis.litPropertyMetadata.set(i,a=new Map),r===`setter`&&((e=Object.create(e)).wrapped=!0),a.set(n.name,e),r===`accessor`){let{name:r}=n;return{set(n){let i=t.get.call(this);t.set.call(this,n),this.requestUpdate(r,i,e,!0,n)},init(t){return t!==void 0&&this.C(r,void 0,e,t),t}}}if(r===`setter`){let{name:r}=n;return function(n){let i=this[r];t.call(this,n),this.requestUpdate(r,i,e,!0,n)}}throw Error(`Unsupported decorator location: `+r)};function T(e){return(t,n)=>typeof n==`object`?Ge(e,t,n):((e,t,n)=>{let r=t.hasOwnProperty(n);return t.constructor.createProperty(n,e),r?Object.getOwnPropertyDescriptor(t,n):void 0})(e,t,n)}function Ke(e){return T({...e,state:!0,attribute:!1})}var qe=(e,t,n)=>(n.configurable=!0,n.enumerable=!0,Reflect.decorate&&typeof t!=`object`&&Object.defineProperty(e,t,n),n);function E(e,t){return(n,r,i)=>{let a=t=>t.renderRoot?.querySelector(e)??null;if(t){let{get:e,set:t}=typeof r==`object`?n:i??(()=>{let e=Symbol();return{get(){return this[e]},set(t){this[e]=t}}})();return qe(n,r,{get(){let n=e.call(this);return n===void 0&&(n=a(this),(n!==null||this.hasUpdated)&&t.call(this,n)),n}})}return qe(n,r,{get(){return a(this)}})}}var Je=o`
  :host {
    display: block;
    /* MD3 doesn't define standard color roles for "success"/"warning"/"info"
       the way it does for "error", so these are this component's own tokens
       (--lit-material-alert-{info,success,warning}-{color,container} and
       --lit-material-alert-on-…-container). Their defaults are fallbacks in
       the rules below, not declarations here: a value declared on :host
       would beat one a page sets on :root or any ancestor, and theming the
       alert would need an element selector. error uses the real
       --md-sys-color-error/-container/-on-error-container tokens. Each pair
       is light-dark() so text stays readable in both schemes. */
  }

  :host([hidden]) {
    display: none;
  }

  .alert {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    box-sizing: border-box;
    padding: 16px;
    border-radius: var(--md-sys-shape-corner-small, 8px);
    color: var(--md-sys-color-on-surface, #1c1b1f);
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
  }

  .icon {
    flex: none;
    display: flex;
  }

  .icon ::slotted(*),
  .icon svg {
    width: 24px;
    height: 24px;
    display: block;
  }

  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  /* .title/.actions are display:contents slots, not wrapping divs — a
     wrapping div's own :empty/:has(::slotted()) can't reliably detect
     absent content (see lit-material-panel and
     lit-material-description-list for the fuller explanation), so an
     unused slot here just has nothing to promote into the flex column
     rather than a stray empty box. Slot exactly one element into "title";
     for multiple "action" buttons, wrap them in a single element yourself
     so they end up in one row instead of stacked in this column. */
  .title,
  .actions {
    display: contents;
  }

  ::slotted([slot="title"]) {
    font-family: var(--md-sys-typescale-title-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-title-medium-size, 1rem);
    font-weight: var(--md-sys-typescale-title-medium-weight, 500);
  }

  ::slotted([slot="action"]) {
    margin-top: 4px;
  }

  .close {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    margin: 0;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: inherit;
    cursor: pointer;
  }

  .close:hover {
    background-color: color-mix(in srgb, currentColor 8%, transparent);
  }

  .close:focus-visible {
    outline: 2px solid currentColor;
    outline-offset: 1px;
  }

  .close svg {
    width: 18px;
    height: 18px;
  }

  :host([variant="info"]) .alert,
  :host(:not([variant])) .alert {
    background-color: var(--lit-material-alert-info-container, light-dark(#d3e3fd, #0a3577));
    color: var(--lit-material-alert-on-info-container, light-dark(#001b3d, #d3e3fd));
  }

  :host([variant="info"]) .icon,
  :host(:not([variant])) .icon {
    color: var(--lit-material-alert-info-color, light-dark(#0b57d0, #a8c7fa));
  }

  :host([variant="success"]) .alert {
    background-color: var(--lit-material-alert-success-container, light-dark(#d0f0d8, #0f5223));
    color: var(--lit-material-alert-on-success-container, light-dark(#04210d, #d0f0d8));
  }

  :host([variant="success"]) .icon {
    color: var(--lit-material-alert-success-color, light-dark(#146c2e, #83da9e));
  }

  :host([variant="warning"]) .alert {
    background-color: var(--lit-material-alert-warning-container, light-dark(#ffe4b3, #5c3f00));
    color: var(--lit-material-alert-on-warning-container, light-dark(#271900, #ffe4b3));
  }

  :host([variant="warning"]) .icon {
    color: var(--lit-material-alert-warning-color, light-dark(#7a4d00, #ffb945));
  }

  :host([variant="error"]) .alert {
    background-color: var(--md-sys-color-error-container, #f9dedc);
    color: var(--md-sys-color-on-error-container, #410e0b);
  }

  :host([variant="error"]) .icon {
    color: var(--md-sys-color-error, #b3261e);
  }
`,Ye=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Xe={info:v`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <line x1="12" y1="11" x2="12" y2="16" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="7.5" r="1.25" fill="currentColor"></circle>
    </svg>
  `,success:v`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <polyline points="7,12.5 10.5,16 17,8.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></polyline>
    </svg>
  `,warning:v`
    <svg viewBox="0 0 24 24">
      <polygon points="12,3 22,20 2,20" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"></polygon>
      <line x1="12" y1="9" x2="12" y2="14" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="17" r="1.15" fill="currentColor"></circle>
    </svg>
  `,error:v`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <line x1="12" y1="7" x2="12" y2="13" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="16.5" r="1.25" fill="currentColor"></circle>
    </svg>
  `},Ze=class extends C{constructor(){super(...arguments),this.variant=`info`,this.dismissible=!1,this.roleIsOwn=!1}static{this.styles=Je}willUpdate(){(!this.hasAttribute(`role`)||this.roleIsOwn)&&(this.setAttribute(`role`,this.variant===`warning`||this.variant===`error`?`alert`:`status`),this.roleIsOwn=!0)}dismiss(){this.hidden=!0,this.dispatchEvent(new Event(`close`,{bubbles:!0}))}render(){return _`
      <div class="alert" part="alert">
        <span class="icon" part="icon" aria-hidden="true">
          <slot name="icon">${Xe[this.variant]}</slot>
        </span>
        <div class="body" part="body">
          <slot name="title" class="title"></slot>
          <div class="description" part="description"><slot></slot></div>
          <slot name="action" class="actions"></slot>
        </div>
        ${this.dismissible?_`
              <button class="close" part="close" type="button" aria-label="Dismiss" @click=${this.dismiss}>
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
                  <line x1="6" y1="18" x2="18" y2="6" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
                </svg>
              </button>
            `:b}
      </div>
    `}};Ye([T()],Ze.prototype,`variant`,void 0),Ye([T({type:Boolean,reflect:!0})],Ze.prototype,`dismissible`,void 0),Ze=Ye([w(`lit-material-alert`)],Ze);var Qe=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},D=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}},$e=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .button {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    box-sizing: border-box;
    height: var(--lit-material-button-height, 40px);
    min-width: 64px;
    padding: 0 var(--lit-material-button-padding-inline, 24px);
    border: none;
    border-radius: var(--md-sys-shape-corner-full, 9999px);
    background-color: transparent;
    cursor: pointer;
    font-family: var(--md-sys-typescale-label-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-large-size, 0.875rem);
    font-weight: var(--md-sys-typescale-label-large-weight, 500);
    line-height: var(--md-sys-typescale-label-large-line-height, 1.25rem);
    letter-spacing: var(--md-sys-typescale-label-large-tracking, 0.00625rem);
    text-decoration: none;
    -webkit-appearance: none;
    appearance: none;
    transition:
      box-shadow 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .button:disabled,
  .button[aria-disabled="true"] {
    cursor: default;
  }

  .state-layer {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background-color: currentColor;
    opacity: 0;
    pointer-events: none;
    transition: opacity 100ms linear;
  }

  .button:hover .state-layer {
    opacity: var(--md-sys-state-hover-state-layer-opacity, 0.08);
  }

  .state-layer[data-pressed] {
    opacity: var(--md-sys-state-pressed-state-layer-opacity, 0.1);
  }

  .focus-ring {
    position: absolute;
    inset: -2px;
    border-radius: inherit;
    outline: 3px solid var(--md-sys-color-secondary, #625b71);
    outline-offset: 2px;
    pointer-events: none;
  }

  .focus-ring[hidden] {
    display: none;
  }

  slot.icon::slotted(*) {
    width: 18px;
    height: 18px;
  }

  .label {
    position: relative;
  }

  /* Filled (default): solid primary container. */
  :host([variant="filled"]) .button,
  :host(:not([variant])) .button {
    background-color: var(--md-sys-color-primary, #6750a4);
    color: var(--md-sys-color-on-primary, #ffffff);
  }

  /* Tonal: solid secondary container, lower emphasis than filled. */
  :host([variant="tonal"]) .button {
    background-color: var(--md-sys-color-secondary-container, #e8def8);
    color: var(--md-sys-color-on-secondary-container, #1d192b);
  }

  /* Elevated: surface-colored container that floats above the page via shadow. */
  :host([variant="elevated"]) .button {
    background-color: var(--md-sys-color-surface-container-low, #f7f2fa);
    color: var(--md-sys-color-primary, #6750a4);
    box-shadow: var(--md-sys-elevation-level1, 0 1px 2px rgba(0, 0, 0, 0.3));
  }

  :host([variant="elevated"]) .button:hover {
    box-shadow: var(--md-sys-elevation-level2, 0 1px 3px rgba(0, 0, 0, 0.3));
  }

  /* Outlined: transparent container with a visible border, for medium emphasis. */
  :host([variant="outlined"]) .button {
    background-color: transparent;
    color: var(--md-sys-color-primary, #6750a4);
    border: 1px solid var(--md-sys-color-outline, #79747e);
  }

  :host([variant="outlined"][disabled]) .button {
    border-color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 12%, transparent);
  }

  /* Text: no container at all, lowest emphasis. */
  :host([variant="text"]) .button {
    background-color: transparent;
    color: var(--md-sys-color-primary, #6750a4);
    padding: 0 var(--lit-material-button-text-padding-inline, 12px);
  }

  /* Disabled treatment is identical across variants: on-surface at fixed opacities. */
  :host([disabled]) .button {
    background-color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 12%, transparent);
    color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 38%, transparent);
    box-shadow: none;
  }

  :host([variant="outlined"][disabled]) .button,
  :host([variant="text"][disabled]) .button {
    background-color: transparent;
  }
`,O=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},et=0,k=class extends C{static{this.styles=$e}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`button`,this.disabled=!1,this.name=``,this.value=``,this.href=``,this.target=``,this.ripple=new Qe(this),this.focusRing=new D(this),this.labelId=`lit-material-button-label-${++et}`,this.internals=this.attachInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}render(){let e=_`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      <slot name="icon" class="icon"></slot>
      <span class="label" id=${this.labelId}><slot></slot></span>
    `;return this.href?_`
        <a
          class="button"
          part="button"
          href=${this.disabled?b:this.href}
          target=${this.target||b}
          rel=${this.target===`_blank`?`noopener noreferrer`:b}
          aria-disabled=${this.disabled?`true`:b}
          aria-labelledby=${this.labelId}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${e}
        </a>
      `:_`
      <button
        class="button"
        part="button"
        type="button"
        ?disabled=${this.disabled}
        name=${this.name||b}
        value=${this.value||b}
        aria-labelledby=${this.labelId}
        @click=${this.handleClick}
      >
        ${e}
      </button>
    `}handleClick(e){this.disabled?(e.preventDefault(),e.stopImmediatePropagation()):this.type===`submit`?this.internals.form?.requestSubmit():this.type===`reset`&&this.internals.form?.reset()}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};O([T()],k.prototype,`variant`,void 0),O([T()],k.prototype,`type`,void 0),O([T({type:Boolean,reflect:!0})],k.prototype,`disabled`,void 0),O([T()],k.prototype,`name`,void 0),O([T()],k.prototype,`value`,void 0),O([T({reflect:!0})],k.prototype,`form`,void 0),O([T()],k.prototype,`href`,void 0),O([T()],k.prototype,`target`,void 0),O([E(`.state-layer`)],k.prototype,`stateLayerElement`,void 0),k=O([w(`lit-material-button`)],k);var tt=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},nt=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}},rt=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .card {
    position: relative;
    display: block;
    box-sizing: border-box;
    width: 100%;
    padding: 16px;
    border-radius: var(--md-sys-shape-corner-medium, 12px);
    text-align: start;
    text-decoration: none;
    color: var(--md-sys-color-on-surface, #1c1b1f);
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
    transition:
      box-shadow 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .card.interactive {
    border: none;
    margin: 0;
    cursor: pointer;
    -webkit-appearance: none;
    appearance: none;
  }

  .card.interactive:disabled,
  .card.interactive[aria-disabled="true"] {
    cursor: default;
  }

  /* Elevated: surface-colored container that floats above the page via shadow. */
  .card.elevated {
    background-color: var(--md-sys-color-surface-container-low, #f7f2fa);
    box-shadow: var(--md-sys-elevation-level1, 0 1px 2px rgba(0, 0, 0, 0.3));
  }

  .card.elevated.interactive:hover {
    box-shadow: var(--md-sys-elevation-level2, 0 1px 3px rgba(0, 0, 0, 0.3));
  }

  /* Filled: solid surface container, no border or shadow. */
  .card.filled {
    background-color: var(--md-sys-color-surface-container-highest, #e6e0e9);
  }

  /* Outlined: transparent surface with a visible border. */
  .card.outlined {
    background-color: var(--md-sys-color-surface, #fffbfe);
    border: 1px solid var(--md-sys-color-outline-variant, #cac4d0);
  }

  .card.outlined.interactive {
    border: 1px solid var(--md-sys-color-outline-variant, #cac4d0);
  }

  .state-layer {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background-color: currentColor;
    opacity: 0;
    pointer-events: none;
    transition: opacity 100ms linear;
  }

  .card.interactive:hover .state-layer {
    opacity: var(--md-sys-state-hover-state-layer-opacity, 0.08);
  }

  .state-layer[data-pressed] {
    opacity: var(--md-sys-state-pressed-state-layer-opacity, 0.1);
  }

  .focus-ring {
    position: absolute;
    inset: -2px;
    border-radius: inherit;
    outline: 3px solid var(--md-sys-color-secondary, #625b71);
    outline-offset: 2px;
    pointer-events: none;
  }

  .focus-ring[hidden] {
    display: none;
  }

  /* Disabled: dim uniformly, drop elevation/state feedback. */
  :host([disabled]) .card {
    box-shadow: none;
  }

  :host([disabled]) .card.elevated {
    background-color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 12%, transparent);
  }

  :host([disabled]) .card {
    color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 38%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    .card,
    .state-layer {
      transition: none;
    }
  }
`,A=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},j=class extends C{static{this.styles=rt}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`elevated`,this.interactive=!1,this.disabled=!1,this.type=`button`,this.name=``,this.value=``,this.href=``,this.target=``,this.ripple=new tt(this),this.focusRing=new nt(this),this.internals=this.attachInternals()}updated(){this.isInteractive&&this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get isInteractive(){return this.interactive||!!this.href}render(){if(!this.isInteractive)return _`<div class="card ${this.variant}" part="card"><slot></slot></div>`;let e=_`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      <slot></slot>
    `;return this.href?_`
        <a
          class="card ${this.variant} interactive"
          part="card"
          href=${this.disabled?b:this.href}
          target=${this.target||b}
          rel=${this.target===`_blank`?`noopener noreferrer`:b}
          aria-disabled=${this.disabled?`true`:b}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${e}
        </a>
      `:_`
      <button
        class="card ${this.variant} interactive"
        part="card"
        type="button"
        ?disabled=${this.disabled}
        name=${this.name||b}
        value=${this.value||b}
        @click=${this.handleClick}
      >
        ${e}
      </button>
    `}handleClick(e){this.disabled?(e.preventDefault(),e.stopImmediatePropagation()):this.type===`submit`?this.internals.form?.requestSubmit():this.type===`reset`&&this.internals.form?.reset()}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};A([T()],j.prototype,`variant`,void 0),A([T({type:Boolean,reflect:!0})],j.prototype,`interactive`,void 0),A([T({type:Boolean,reflect:!0})],j.prototype,`disabled`,void 0),A([T()],j.prototype,`type`,void 0),A([T()],j.prototype,`name`,void 0),A([T()],j.prototype,`value`,void 0),A([T({reflect:!0})],j.prototype,`form`,void 0),A([T()],j.prototype,`href`,void 0),A([T()],j.prototype,`target`,void 0),A([E(`.state-layer`)],j.prototype,`stateLayerElement`,void 0),j=A([w(`lit-material-card`)],j);var it=o`
  :host {
    display: contents;
  }

  .drawer {
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    gap: 4px;
    width: 360px;
    max-width: 100%;
    height: 100%;
    padding: 12px;
    background-color: var(--md-sys-color-surface, #fffbfe);
    color: var(--md-sys-color-on-surface, #1c1b1f);
    /* Not part of the tab order for "modal" (see the component for why it's
       the default showModal() focus target) — no visible focus indicator
       of its own. */
    outline: none;
  }

  .header {
    padding: 16px 16px 8px;
  }

  .header:empty {
    display: none;
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
    overflow-y: auto;
  }

  /* Modal variant: same .drawer content, wrapped in a native <dialog> so the
     scrim, Escape-to-close, and focus trap all come from the browser. */
  .dialog {
    margin: 0;
    padding: 0;
    border: none;
    inset: 0;
    width: 100%;
    height: 100%;
    max-width: 100%;
    max-height: 100%;
    background: transparent;
    color: inherit;
  }

  .dialog[open] {
    display: flex;
    justify-content: flex-start;
  }

  :host([position="end"]) .dialog[open] {
    justify-content: flex-end;
  }

  .dialog::backdrop {
    background-color: rgba(0, 0, 0, 0.32);
  }

  .dialog .drawer {
    box-shadow: var(
      --md-sys-elevation-level1,
      0 1px 2px rgba(0, 0, 0, 0.3)
    );
  }
`,M=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},N=class extends C{static{this.styles=it}constructor(){super(),this.variant=`standard`,this.position=`start`,this.selected=-1,this.open=!1,this.disableBackdropClose=!1,this.reopenOnClose=!1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected(),e.has(`open`)&&this.variant===`modal`&&this.dialogElement&&(this.open&&!this.dialogElement.open?this.dialogElement.showModal():!this.open&&this.dialogElement.open&&this.dialogElement.close())}show(){this.open=!0}close(){this.open=!1}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-drawer-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}renderNav(){let e=this.variant===`modal`;return _`
      <nav
        class="drawer ${this.variant}"
        part="drawer"
        tabindex=${e?`-1`:b}
        ?autofocus=${e}
      >
        <div class="header" part="header"><slot name="header"></slot></div>
        <div class="content" part="content"><slot @slotchange=${this.handleSlotChange}></slot></div>
      </nav>
    `}render(){return this.variant===`modal`?_`
      <dialog
        class="dialog"
        part="dialog"
        @close=${this.handleClose}
        @cancel=${this.handleCancel}
        @click=${this.handleBackdropClick}
      >
        ${this.renderNav()}
      </dialog>
    `:this.renderNav()}handleClose(){this.reopenOnClose?(this.reopenOnClose=!1,this.dialogElement?.showModal()):(this.open=!1,this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0})))}handleCancel(e){this.dispatchEvent(new Event(`cancel`,{bubbles:!0,cancelable:!0,composed:!0}))||(e.preventDefault(),this.reopenOnClose=!0)}handleBackdropClick(e){!this.disableBackdropClose&&e.target===this.dialogElement&&this.dialogElement?.close()}};M([T()],N.prototype,`variant`,void 0),M([T({reflect:!0})],N.prototype,`position`,void 0),M([T({type:Number})],N.prototype,`selected`,void 0),M([T({type:Boolean,reflect:!0})],N.prototype,`open`,void 0),M([T({type:Boolean,attribute:`disable-backdrop-close`})],N.prototype,`disableBackdropClose`,void 0),M([E(`dialog`)],N.prototype,`dialogElement`,void 0),N=M([w(`lit-material-navigation-drawer`)],N);var at=o`
  :host {
    display: block;
    outline: none;
    -webkit-tap-highlight-color: transparent;
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    box-sizing: border-box;
    width: 100%;
    min-height: 56px;
    padding-inline: 16px;
    border: none;
    border-radius: var(--md-sys-shape-corner-full, 28px);
    margin: 0;
    background-color: transparent;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    cursor: pointer;
    text-align: start;
    text-decoration: none;
    font-family: var(--md-sys-typescale-label-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-large-size, 0.875rem);
    font-weight: var(--md-sys-typescale-label-large-weight, 500);
    -webkit-appearance: none;
    appearance: none;
    transition: background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .item:disabled,
  .item[aria-disabled="true"] {
    cursor: default;
  }

  :host([selected]) .item {
    background-color: var(--md-sys-color-secondary-container, #e8def8);
    color: var(--md-sys-color-on-secondary-container, #1d192b);
  }

  slot.icon::slotted(*) {
    display: block;
    width: 24px;
    height: 24px;
  }

  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  slot.badge::slotted(*) {
    display: block;
  }

  .state-layer {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background-color: currentColor;
    opacity: 0;
    pointer-events: none;
    transition: opacity 100ms linear;
  }

  .item:hover .state-layer {
    opacity: var(--md-sys-state-hover-state-layer-opacity, 0.08);
  }

  .state-layer[data-pressed] {
    opacity: var(--md-sys-state-pressed-state-layer-opacity, 0.1);
  }

  .focus-ring {
    position: absolute;
    inset: -2px;
    border-radius: inherit;
    outline: 3px solid var(--md-sys-color-secondary, #625b71);
    outline-offset: 2px;
    pointer-events: none;
  }

  .focus-ring[hidden] {
    display: none;
  }

  :host([disabled]) .item {
    color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 38%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    .item,
    .state-layer {
      transition: none;
    }
  }
`,P=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},F=class extends C{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Qe(this),this.focusRing=new D(this)}static{this.styles=at}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return _`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      <slot name="icon" class="icon"></slot>
      <span class="label"><slot></slot></span>
      <slot name="badge" class="badge"></slot>
    `}render(){let e=this.selected?`page`:b;return this.href?_`
        <a
          class="item"
          part="item"
          href=${this.disabled?b:this.href}
          target=${this.target||b}
          rel=${this.target===`_blank`?`noopener noreferrer`:b}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:b}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:_`
      <button
        class="item"
        part="item"
        type="button"
        ?disabled=${this.disabled}
        aria-current=${e}
        @click=${this.handleClick}
      >
        ${this.content}
      </button>
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};P([T({type:Boolean,reflect:!0})],F.prototype,`selected`,void 0),P([T({type:Boolean,reflect:!0})],F.prototype,`disabled`,void 0),P([T()],F.prototype,`href`,void 0),P([T()],F.prototype,`target`,void 0),P([E(`.state-layer`)],F.prototype,`stateLayerElement`,void 0),F=P([w(`lit-material-navigation-drawer-item`)],F);var ot=o`
  :host {
    display: block;
    box-sizing: border-box;
  }

  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    box-sizing: border-box;
    width: 80px;
    height: 100%;
    padding-block: 12px;
    background-color: var(--md-sys-color-surface, #fffbfe);
    color: var(--md-sys-color-on-surface, #1c1b1f);
  }

  .fab {
    flex: none;
    margin-block-end: 12px;
  }

  .fab:empty {
    display: none;
  }

  .items {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    min-height: 0;
    overflow-y: auto;
  }

  .items.top {
    justify-content: flex-start;
  }

  .items.center {
    justify-content: center;
  }

  .items.bottom {
    justify-content: flex-end;
  }
`,st=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ct=class extends C{static{this.styles=ot}constructor(){super(),this.alignment=`top`,this.selected=-1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected()}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-rail-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}render(){return _`
      <div class="rail" part="rail">
        <div class="fab" part="fab"><slot name="fab"></slot></div>
        <div class="items ${this.alignment}" part="items">
          <slot @slotchange=${this.handleSlotChange}></slot>
        </div>
      </div>
    `}};st([T()],ct.prototype,`alignment`,void 0),st([T({type:Number})],ct.prototype,`selected`,void 0),ct=st([w(`lit-material-navigation-rail`)],ct);var lt=o`
  :host {
    display: block;
    outline: none;
    -webkit-tap-highlight-color: transparent;
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .item {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    box-sizing: border-box;
    width: 100%;
    min-height: 56px;
    padding-block: 4px;
    border: none;
    margin: 0;
    background-color: transparent;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    cursor: pointer;
    text-decoration: none;
    font-family: var(--md-sys-typescale-label-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-medium-size, 0.75rem);
    font-weight: var(--md-sys-typescale-label-medium-weight, 500);
    -webkit-appearance: none;
    appearance: none;
  }

  .item:disabled,
  .item[aria-disabled="true"] {
    cursor: default;
  }

  :host([selected]) .item {
    color: var(--md-sys-color-on-surface, #1c1b1f);
  }

  .icon-container {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 32px;
    border-radius: var(--md-sys-shape-corner-full, 16px);
    transition: background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  :host([selected]) .icon-container {
    background-color: var(--md-sys-color-secondary-container, #e8def8);
  }

  slot.icon::slotted(*) {
    display: block;
    width: 24px;
    height: 24px;
  }

  slot.badge::slotted(*) {
    position: absolute;
    top: -4px;
    inset-inline-end: 8px;
  }

  .label {
    max-width: 100%;
    overflow: hidden;
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state-layer {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background-color: currentColor;
    opacity: 0;
    pointer-events: none;
    transition: opacity 100ms linear;
  }

  .item:hover .state-layer {
    opacity: var(--md-sys-state-hover-state-layer-opacity, 0.08);
  }

  .state-layer[data-pressed] {
    opacity: var(--md-sys-state-pressed-state-layer-opacity, 0.1);
  }

  .focus-ring {
    position: absolute;
    inset: -2px;
    border-radius: inherit;
    outline: 3px solid var(--md-sys-color-secondary, #625b71);
    outline-offset: 2px;
    pointer-events: none;
  }

  .focus-ring[hidden] {
    display: none;
  }

  :host([disabled]) .item {
    color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 38%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    .icon-container,
    .state-layer {
      transition: none;
    }
  }
`,I=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},L=class extends C{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Qe(this),this.focusRing=new D(this)}static{this.styles=lt}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return _`
      <span class="icon-container" part="icon-container">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="state-layer" part="ripple"></div>
        <slot name="icon" class="icon"></slot>
        <slot name="badge" class="badge"></slot>
      </span>
      <span class="label"><slot></slot></span>
    `}render(){let e=this.selected?`page`:b;return this.href?_`
        <a
          class="item"
          part="item"
          href=${this.disabled?b:this.href}
          target=${this.target||b}
          rel=${this.target===`_blank`?`noopener noreferrer`:b}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:b}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:_`
      <button
        class="item"
        part="item"
        type="button"
        ?disabled=${this.disabled}
        aria-current=${e}
        @click=${this.handleClick}
      >
        ${this.content}
      </button>
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};I([T({type:Boolean,reflect:!0})],L.prototype,`selected`,void 0),I([T({type:Boolean,reflect:!0})],L.prototype,`disabled`,void 0),I([T()],L.prototype,`href`,void 0),I([T()],L.prototype,`target`,void 0),I([E(`.state-layer`)],L.prototype,`stateLayerElement`,void 0),L=I([w(`lit-material-navigation-rail-item`)],L);var ut=o`
  :host {
    display: block;
    box-sizing: border-box;
  }

  .bar {
    display: flex;
    flex-direction: row;
    align-items: stretch;
    box-sizing: border-box;
    width: 100%;
    height: 80px;
    background-color: var(--md-sys-color-surface-container, #f3edf7);
    color: var(--md-sys-color-on-surface, #1c1b1f);
  }

  ::slotted(lit-material-navigation-bar-item) {
    flex: 1;
  }
`,dt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ft=class extends C{static{this.styles=ut}constructor(){super(),this.selected=-1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected()}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-bar-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}render(){return _`
      <div class="bar" part="bar">
        <slot @slotchange=${this.handleSlotChange}></slot>
      </div>
    `}};dt([T({type:Number})],ft.prototype,`selected`,void 0),ft=dt([w(`lit-material-navigation-bar`)],ft);var pt=o`
  :host {
    display: block;
    flex: 1;
    height: 100%;
    outline: none;
    -webkit-tap-highlight-color: transparent;
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .item {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    padding-block: 12px 16px;
    border: none;
    margin: 0;
    background-color: transparent;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    cursor: pointer;
    text-decoration: none;
    font-family: var(--md-sys-typescale-label-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-medium-size, 0.75rem);
    font-weight: var(--md-sys-typescale-label-medium-weight, 500);
    -webkit-appearance: none;
    appearance: none;
  }

  .item:disabled,
  .item[aria-disabled="true"] {
    cursor: default;
  }

  :host([selected]) .item {
    color: var(--md-sys-color-on-surface, #1c1b1f);
  }

  .icon-container {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 64px;
    height: 32px;
    border-radius: var(--md-sys-shape-corner-full, 16px);
    transition: background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  :host([selected]) .icon-container {
    background-color: var(--md-sys-color-secondary-container, #e8def8);
  }

  slot.icon::slotted(*) {
    display: block;
    width: 24px;
    height: 24px;
  }

  slot.badge::slotted(*) {
    position: absolute;
    top: -4px;
    inset-inline-end: 8px;
  }

  .label {
    max-width: 100%;
    overflow: hidden;
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state-layer {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background-color: currentColor;
    opacity: 0;
    pointer-events: none;
    transition: opacity 100ms linear;
  }

  .item:hover .state-layer {
    opacity: var(--md-sys-state-hover-state-layer-opacity, 0.08);
  }

  .state-layer[data-pressed] {
    opacity: var(--md-sys-state-pressed-state-layer-opacity, 0.1);
  }

  .focus-ring {
    position: absolute;
    inset: -2px;
    border-radius: inherit;
    outline: 3px solid var(--md-sys-color-secondary, #625b71);
    outline-offset: 2px;
    pointer-events: none;
  }

  .focus-ring[hidden] {
    display: none;
  }

  :host([disabled]) .item {
    color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 38%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    .icon-container,
    .state-layer {
      transition: none;
    }
  }
`,R=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},z=class extends C{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Qe(this),this.focusRing=new D(this)}static{this.styles=pt}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return _`
      <span class="icon-container" part="icon-container">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="state-layer" part="ripple"></div>
        <slot name="icon" class="icon"></slot>
        <slot name="badge" class="badge"></slot>
      </span>
      <span class="label"><slot></slot></span>
    `}render(){let e=this.selected?`page`:b;return this.href?_`
        <a
          class="item"
          part="item"
          href=${this.disabled?b:this.href}
          target=${this.target||b}
          rel=${this.target===`_blank`?`noopener noreferrer`:b}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:b}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:_`
      <button
        class="item"
        part="item"
        type="button"
        ?disabled=${this.disabled}
        aria-current=${e}
        @click=${this.handleClick}
      >
        ${this.content}
      </button>
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};R([T({type:Boolean,reflect:!0})],z.prototype,`selected`,void 0),R([T({type:Boolean,reflect:!0})],z.prototype,`disabled`,void 0),R([T()],z.prototype,`href`,void 0),R([T()],z.prototype,`target`,void 0),R([E(`.state-layer`)],z.prototype,`stateLayerElement`,void 0),z=R([w(`lit-material-navigation-bar-item`)],z);var mt=o`
  @keyframes lit-material-skeleton-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }

  @keyframes lit-material-skeleton-wave {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }

  :host {
    display: block;
  }

  .skeleton {
    position: relative;
    display: block;
    overflow: hidden;
    width: 100%;
    background-color: var(--md-sys-color-surface-container-highest, #e6e0e9);
  }

  /* Text (default): a rounded bar sized to the surrounding text's line-height. */
  :host(:not([variant])) .skeleton,
  :host([variant="text"]) .skeleton {
    height: 1em;
    border-radius: var(--md-sys-shape-corner-extra-small, 4px);
  }

  :host([variant="circular"]) .skeleton {
    width: 40px;
    height: 40px;
    border-radius: 50%;
  }

  :host([variant="rectangular"]) .skeleton {
    height: 120px;
  }

  :host([variant="rounded"]) .skeleton {
    height: 120px;
    border-radius: var(--md-sys-shape-corner-medium, 12px);
  }

  :host([animation="pulse"]) .skeleton,
  :host(:not([animation])) .skeleton {
    animation: lit-material-skeleton-pulse 1.5s ease-in-out infinite;
  }

  :host([animation="wave"]) .skeleton::after {
    content: "";
    position: absolute;
    inset: 0;
    transform: translateX(-100%);
    background: linear-gradient(
      90deg,
      transparent,
      color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 10%, transparent),
      transparent
    );
    animation: lit-material-skeleton-wave 1.6s linear infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    .skeleton,
    .skeleton::after {
      animation: none;
    }
  }
`,B=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},V=class extends C{constructor(){super(...arguments),this.variant=`text`,this.animation=`pulse`,this.width=``,this.height=``}static{this.styles=mt}render(){return _`<span class="skeleton" part="skeleton" style=${`${this.width?`width: ${this.width};`:``}${this.height?`height: ${this.height};`:``}`||b} aria-hidden="true"></span>`}};B([T({reflect:!0})],V.prototype,`variant`,void 0),B([T({reflect:!0})],V.prototype,`animation`,void 0),B([T()],V.prototype,`width`,void 0),B([T()],V.prototype,`height`,void 0),V=B([w(`lit-material-skeleton`)],V);var ht=o`
  :host {
    position: fixed;
    left: 50%;
    bottom: 24px;
    transform: translateX(-50%);
    margin: 0;
    padding: 6px 8px 6px 16px;
    border: none;
    box-sizing: border-box;
    display: none;
    align-items: center;
    gap: 8px;
    min-height: 48px;
    max-width: min(560px, calc(100vw - 32px));
    border-radius: var(--md-sys-shape-corner-extra-small, 4px);
    background-color: var(--md-sys-color-inverse-surface, #313033);
    color: var(--md-sys-color-inverse-on-surface, #f4eff4);
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
    box-shadow: var(
      --md-sys-elevation-level3,
      0 1px 3px 0 rgba(0, 0, 0, 0.3),
      0 4px 8px 3px rgba(0, 0, 0, 0.15)
    );
  }

  :host(:popover-open) {
    display: flex;
  }

  .content {
    flex: 1;
    min-width: 0;
  }

  ::slotted([slot="action"]) {
    flex: none;
    color: var(--md-sys-color-inverse-primary, #d0bcff);
  }

  .close {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 40px;
    height: 40px;
    padding: 0;
    border: none;
    background: transparent;
    border-radius: 50%;
    color: inherit;
    cursor: pointer;
    -webkit-appearance: none;
    appearance: none;
  }

  .close:hover {
    background-color: color-mix(in srgb, currentColor 8%, transparent);
  }

  .close:focus-visible {
    outline: 2px solid var(--md-sys-color-inverse-primary, #d0bcff);
    outline-offset: -2px;
  }

  .close svg {
    width: 18px;
    height: 18px;
    pointer-events: none;
  }

  .close path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
  }
`,gt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},H=class extends C{static{this.styles=ht}constructor(){super(),this.open=!1,this.duration=5e3,this.closable=!1,this.pauseTimer=()=>{this.open&&this.clearTimer()},this.resumeTimer=()=>{this.open&&this.startTimer()},this.handleClick=e=>{e.target.closest(`[slot="action"]`)&&this.close()},this.addEventListener(`click`,this.handleClick),this.addEventListener(`pointerenter`,this.pauseTimer),this.addEventListener(`pointerleave`,this.resumeTimer),this.addEventListener(`focusin`,this.pauseTimer),this.addEventListener(`focusout`,this.resumeTimer)}connectedCallback(){super.connectedCallback(),this.setAttribute(`popover`,`manual`),this.setAttribute(`role`,`status`),this.setAttribute(`aria-live`,`polite`),this.setAttribute(`aria-atomic`,`true`)}disconnectedCallback(){super.disconnectedCallback(),this.clearTimer()}show(){this.open=!0}close(){this.open=!1}updated(e){e.has(`open`)&&(this.open?(this.matches(`:popover-open`)||this.showPopover(),this.startTimer()):this.matches(`:popover-open`)&&(this.clearTimer(),this.hidePopover(),this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0}))))}render(){return _`
      <div class="content" part="snackbar">
        <slot></slot>
      </div>
      <slot name="action" class="action"></slot>
      ${this.closable?_`
            <button class="close" part="close-button" type="button" aria-label="Dismiss" @click=${this.close}>
              <svg viewBox="0 0 18 18" aria-hidden="true">
                <path d="M5 5 L13 13 M13 5 L5 13"></path>
              </svg>
            </button>
          `:b}
    `}startTimer(){this.clearTimer(),this.duration>0&&(this.timeoutId=window.setTimeout(()=>this.close(),this.duration))}clearTimer(){this.timeoutId!==void 0&&(window.clearTimeout(this.timeoutId),this.timeoutId=void 0)}};gt([T({type:Boolean,reflect:!0})],H.prototype,`open`,void 0),gt([T({type:Number})],H.prototype,`duration`,void 0),gt([T({type:Boolean})],H.prototype,`closable`,void 0),H=gt([w(`lit-material-snackbar`)],H);var _t=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},vt=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}},yt=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .switch {
    position: relative;
    display: inline-flex;
    align-items: center;
    box-sizing: border-box;
    width: 52px;
    height: 40px;
    cursor: pointer;
    color: var(--md-sys-color-on-surface, #1c1b1f);
  }

  .track {
    position: absolute;
    top: 50%;
    left: 0;
    width: 52px;
    height: 32px;
    box-sizing: border-box;
    border-radius: var(--md-sys-shape-corner-full, 9999px);
    transform: translateY(-50%);
    background-color: var(--md-sys-color-surface-container-highest, #e6e0e9);
    border: 2px solid var(--md-sys-color-outline, #79747e);
    pointer-events: none;
    transition:
      background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      border-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .switch.checked .track {
    background-color: var(--md-sys-color-primary, #6750a4);
    border-color: transparent;
  }

  .thumb-wrap {
    position: absolute;
    top: 50%;
    left: 0;
    width: 40px;
    height: 40px;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
    transition: left 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .switch.checked .thumb-wrap {
    left: 12px;
  }

  .state-layer {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background-color: currentColor;
    opacity: 0;
    transition: opacity 100ms linear;
  }

  .switch:hover .state-layer {
    opacity: var(--md-sys-state-hover-state-layer-opacity, 0.08);
  }

  .state-layer[data-pressed] {
    opacity: var(--md-sys-state-pressed-state-layer-opacity, 0.1);
  }

  .switch.checked .state-layer {
    color: var(--md-sys-color-primary, #6750a4);
  }

  .thumb {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background-color: var(--md-sys-color-outline, #79747e);
    transition:
      width 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      height 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .switch.checked .thumb {
    width: 24px;
    height: 24px;
    background-color: var(--md-sys-color-on-primary, #ffffff);
  }

  .icon ::slotted(*) {
    width: 16px;
    height: 16px;
    display: block;
    pointer-events: none;
  }

  .focus-ring {
    position: absolute;
    inset: -2px;
    border-radius: var(--md-sys-shape-corner-full, 9999px);
    outline: 3px solid var(--md-sys-color-secondary, #625b71);
    outline-offset: 2px;
    pointer-events: none;
  }

  .focus-ring[hidden] {
    display: none;
  }

  .native-control {
    position: absolute;
    inset: 0;
    margin: 0;
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    opacity: 0;
    cursor: inherit;
    -webkit-appearance: none;
    appearance: none;
  }

  .native-control:disabled {
    cursor: default;
  }

  /* Error: red outline (unselected) or red track/dark thumb (selected). */
  :host([error]) .track {
    border-color: var(--md-sys-color-error, #b3261e);
  }

  :host([error]) .switch.checked .track {
    background-color: var(--md-sys-color-error, #b3261e);
    border-color: transparent;
  }

  :host([error]) .switch.checked .state-layer {
    color: var(--md-sys-color-error, #b3261e);
  }

  /* Disabled: dim everything uniformly. */
  :host([disabled]) .track,
  :host([disabled]) .thumb {
    opacity: 0.38;
  }

  @media (prefers-reduced-motion: reduce) {
    .track,
    .thumb-wrap,
    .thumb,
    .state-layer {
      transition: none;
    }
  }
`,U=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},W=class extends C{static{this.styles=yt}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.checked=!1,this.disabled=!1,this.error=!1,this.required=!1,this.name=``,this.value=`on`,this.ariaLabel=null,this.ariaLabelledBy=null,this.ripple=new _t(this),this.focusRing=new vt(this),this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement),this.syncInternals()}updated(e){(e.has(`checked`)||e.has(`required`)||e.has(`error`))&&this.syncInternals()}syncInternals(){if(this.internals.setFormValue(this.checked?this.value:null),this.inputElement){let e=this.error?this.inputElement.validationMessage||`Invalid`:this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){return _`
      <div class="switch ${this.checked?`checked`:``}" part="switch">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="track" part="track">
          <div class="thumb-wrap">
            <div class="state-layer" part="ripple"></div>
            <div class="thumb" part="thumb">
              ${this.checked?_`<slot name="selected-icon" class="icon"></slot>`:_`<slot name="icon" class="icon"></slot>`}
            </div>
          </div>
        </div>
        <input
          class="native-control"
          part="input"
          type="checkbox"
          role="switch"
          .checked=${this.checked}
          ?disabled=${this.disabled}
          ?required=${this.required}
          name=${this.name||b}
          value=${this.value||b}
          aria-invalid=${this.error?`true`:b}
          aria-label=${this.ariaLabel||b}
          aria-labelledby=${this.ariaLabelledBy||b}
          @change=${this.handleChange}
        />
      </div>
    `}handleChange(e){let t=e.target;this.checked=t.checked,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}formResetCallback(){this.checked=this.hasAttribute(`checked`),this.syncInternals()}formStateRestoreCallback(e){this.checked=e!==null}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};U([T({type:Boolean})],W.prototype,`checked`,void 0),U([T({type:Boolean,reflect:!0})],W.prototype,`disabled`,void 0),U([T({type:Boolean,reflect:!0})],W.prototype,`error`,void 0),U([T({type:Boolean})],W.prototype,`required`,void 0),U([T()],W.prototype,`name`,void 0),U([T()],W.prototype,`value`,void 0),U([T({reflect:!0})],W.prototype,`form`,void 0),U([T({attribute:`aria-label`})],W.prototype,`ariaLabel`,void 0),U([T({attribute:`aria-labelledby`})],W.prototype,`ariaLabelledBy`,void 0),U([E(`.native-control`)],W.prototype,`inputElement`,void 0),U([E(`.state-layer`)],W.prototype,`stateLayerElement`,void 0),W=U([w(`lit-material-switch`)],W);var bt=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
    --notch-color: var(--md-sys-color-surface, #fffbfe);
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .text-field {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-width: 210px;
  }

  .container {
    position: relative;
    display: inline-flex;
    align-items: stretch;
    box-sizing: border-box;
    height: 56px;
    width: 100%;
    color: var(--md-sys-color-on-surface, #1c1b1f);
    cursor: text;
  }

  .leading-icon,
  .trailing-icon {
    display: inline-flex;
    align-items: center;
    flex: none;
    align-self: center;
    color: var(--md-sys-color-on-surface-variant, #49454f);
  }

  slot.leading-icon::slotted(*),
  slot.trailing-icon::slotted(*) {
    width: 24px;
    height: 24px;
    display: block;
    pointer-events: none;
  }

  .leading-icon {
    margin-inline: 12px 8px;
  }

  .trailing-icon {
    margin-inline: 8px 12px;
  }

  .field {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  .input-wrapper {
    position: relative;
    display: flex;
    align-items: center;
    align-self: stretch;
    flex: 1;
    min-width: 0;
  }

  .prefix,
  .suffix {
    display: inline-flex;
    align-items: center;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    font-family: var(--md-sys-typescale-label-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-large-size, 0.875rem);
    line-height: var(--md-sys-typescale-label-large-line-height, 1.25rem);
    white-space: nowrap;
  }

  .prefix {
    margin-inline-end: 4px;
  }

  .suffix {
    margin-inline-start: 4px;
  }

  .label {
    position: absolute;
    inset-inline-start: 0;
    top: 50%;
    transform: translateY(-50%);
    max-width: calc(100% - 8px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    pointer-events: none;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    font-family: var(--md-sys-typescale-label-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-large-size, 1rem);
    line-height: var(--md-sys-typescale-label-large-line-height, 1.25rem);
    transition:
      top 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      transform 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      font-size 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .input {
    position: relative;
    width: 100%;
    box-sizing: border-box;
    margin: 0;
    padding: 0;
    border: none;
    background: transparent;
    outline: none;
    color: inherit;
    font-family: inherit;
    font-size: var(--md-sys-typescale-label-large-size, 1rem);
    line-height: var(--md-sys-typescale-label-large-line-height, 1.25rem);
    -webkit-appearance: none;
    appearance: none;
  }

  /* Float the label when focused, holding a value, or when a placeholder is set. */
  .container.floated .label {
    top: 8px;
    transform: translateY(0);
    font-size: var(--md-sys-typescale-label-medium-size, 0.75rem);
    line-height: var(--md-sys-typescale-label-medium-line-height, 1rem);
  }

  .container.focused .label {
    color: var(--md-sys-color-primary, #6750a4);
  }

  /* Filled/default: once the label floats to the top, drop the value row
     (input + prefix/suffix) to the bottom of the field so it no longer
     sits underneath the label. Outlined doesn't need this: its floated
     label notches the outline instead of occupying space inside it. */
  :host([variant="filled"]) .container.floated .field,
  :host(:not([variant])) .container.floated .field {
    align-items: flex-end;
    padding-bottom: 8px;
  }

  :host([variant="filled"]) .container.floated .input-wrapper,
  :host(:not([variant])) .container.floated .input-wrapper {
    align-items: flex-end;
  }

  /* Filled (default): solid surface container with a bottom active indicator. */
  :host([variant="filled"]) .container,
  :host(:not([variant])) .container {
    background-color: var(--md-sys-color-surface-container-highest, #e6e0e9);
    border-radius: var(--md-sys-shape-corner-extra-small, 4px) var(--md-sys-shape-corner-extra-small, 4px) 0 0;
    padding-inline: 16px;
  }

  :host([variant="filled"]) .container::after,
  :host(:not([variant])) .container::after {
    content: "";
    position: absolute;
    inset-inline: 0;
    bottom: 0;
    height: 1px;
    background-color: var(--md-sys-color-on-surface-variant, #49454f);
    transition: height 100ms linear, background-color 100ms linear;
  }

  :host([variant="filled"]) .container.focused::after,
  :host(:not([variant])) .container.focused::after {
    height: 2px;
    background-color: var(--md-sys-color-primary, #6750a4);
  }

  /* Outlined: transparent container with a full outline; the label notches it. */
  :host([variant="outlined"]) .container {
    background-color: transparent;
    border: 1px solid var(--md-sys-color-outline, #79747e);
    border-radius: var(--md-sys-shape-corner-extra-small, 4px);
    padding-inline: 16px;
    transition: border-color 100ms linear, border-width 100ms linear;
  }

  :host([variant="outlined"]) .container.focused {
    border-color: var(--md-sys-color-primary, #6750a4);
    border-width: 2px;
  }

  /* Notch the outline for the floated label. The label carries a background
     matching whatever sits behind the field so the border reads as broken. */
  :host([variant="outlined"]) .container.floated .label {
    top: 0;
    transform: translateY(-50%);
    padding-inline: 4px;
    margin-inline-start: -4px;
    background-color: var(--notch-color);
  }

  /* Error state: red indicator + red label/supporting text. */
  :host([error]) .container.focused .label,
  :host([error]) .label {
    color: var(--md-sys-color-error, #b3261e);
  }

  :host([error]) .container::after {
    height: 2px;
    background-color: var(--md-sys-color-error, #b3261e);
  }

  :host([error][variant="outlined"]) .container,
  :host([error]:not([variant])) .container {
    border-color: var(--md-sys-color-error, #b3261e);
  }

  /* Disabled: dim everything; filled loses its indicator. */
  :host([disabled]) .container {
    opacity: 0.38;
  }

  :host([disabled][variant="filled"]) .container::after,
  :host([disabled]:not([variant])) .container::after {
    height: 1px;
    background-color: var(--md-sys-color-on-surface, #1c1b1f);
  }

  :host([disabled]) .input {
    cursor: default;
  }

  /* Supporting text + character counter row. */
  .supporting {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 16px 0;
    min-height: 1rem;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    font-family: var(--md-sys-typescale-label-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-medium-size, 0.75rem);
    line-height: var(--md-sys-typescale-label-medium-line-height, 1rem);
  }

  .supporting-text {
    flex: 1;
  }

  .counter {
    flex: none;
  }

  :host([error]) .supporting {
    color: var(--md-sys-color-error, #b3261e);
  }

  @media (prefers-reduced-motion: reduce) {
    .label,
    .container,
    .container::after {
      transition: none;
    }
  }
`,G=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},xt=0,K=class extends C{static{this.styles=bt}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`text`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.prefix=``,this.suffix=``,this.step=``,this.pattern=``,this.autocomplete=``,this.inputmode=``,this.name=``,this.focused=!1,this.touched=!1,this.focusRing=new D(this),this.labelId=`lit-material-text-field-label-${++xt}`,this.supportingId=`lit-material-text-field-supporting-${xt}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`pattern`)||e.has(`minlength`)||e.has(`maxlength`)||e.has(`min`)||e.has(`max`)||e.has(`type`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.inputElement?!1:!this.inputElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.inputElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.inputElement){let e=this.isError&&this.errorText||this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){let e=this.floated,t=this.isError;return _`
      <div class="text-field" part="text-field">
        <div
          class="container ${e?`floated`:``} ${this.focused?`focused`:``}"
          part="container"
        >
          <slot name="leading-icon" class="leading-icon"></slot>
          <div class="field">
            ${this.prefix?_`<span class="prefix">${this.prefix}</span>`:b}
            <div class="input-wrapper">
              ${this.label?_`<label class="label" part="label" id=${this.labelId} for="input">${this.label}</label>`:b}
              <input
                class="input"
                part="input"
                id="input"
                .value=${this.value}
                type=${this.type||`text`}
                placeholder=${this.placeholder||b}
                ?disabled=${this.disabled}
                ?readonly=${this.readonly}
                ?required=${this.required}
                autocomplete=${this.autocomplete||b}
                inputmode=${this.inputmode||b}
                min=${this.min??b}
                max=${this.max??b}
                minlength=${this.minlength??b}
                maxlength=${this.maxlength??b}
                step=${this.step||b}
                pattern=${this.pattern||b}
                name=${this.name||b}
                aria-labelledby=${this.label?this.labelId:b}
                aria-describedby=${this.supportingMessage?this.supportingId:b}
                aria-invalid=${t?`true`:b}
                @input=${this.handleInput}
                @change=${this.handleChange}
                @focus=${this.handleFocus}
                @blur=${this.handleBlur}
              />
            </div>
            ${this.suffix?_`<span class="suffix">${this.suffix}</span>`:b}
          </div>
          <slot name="trailing-icon" class="trailing-icon"></slot>
        </div>
        ${this.supportingMessage||this.maxlength!=null?_`
              <div class="supporting" part="supporting">
                <span class="supporting-text" id=${this.supportingId}>
                  ${this.supportingMessage}
                </span>
                ${this.maxlength==null?b:_`<span class="counter">${this.value.length} / ${this.maxlength}</span>`}
              </div>
            `:b}
      </div>
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};G([T()],K.prototype,`variant`,void 0),G([T()],K.prototype,`type`,void 0),G([T()],K.prototype,`value`,void 0),G([T()],K.prototype,`label`,void 0),G([T()],K.prototype,`placeholder`,void 0),G([T({attribute:`supporting-text`})],K.prototype,`supportingText`,void 0),G([T({attribute:`error-text`})],K.prototype,`errorText`,void 0),G([T({type:Boolean,reflect:!0})],K.prototype,`error`,void 0),G([T({type:Boolean,reflect:!0})],K.prototype,`disabled`,void 0),G([T({type:Boolean})],K.prototype,`required`,void 0),G([T({type:Boolean})],K.prototype,`readonly`,void 0),G([T()],K.prototype,`prefix`,void 0),G([T()],K.prototype,`suffix`,void 0),G([T({type:Number})],K.prototype,`min`,void 0),G([T({type:Number})],K.prototype,`max`,void 0),G([T({type:Number})],K.prototype,`minlength`,void 0),G([T({type:Number})],K.prototype,`maxlength`,void 0),G([T()],K.prototype,`step`,void 0),G([T()],K.prototype,`pattern`,void 0),G([T()],K.prototype,`autocomplete`,void 0),G([T()],K.prototype,`inputmode`,void 0),G([T()],K.prototype,`name`,void 0),G([T({reflect:!0})],K.prototype,`form`,void 0),G([E(`.input`)],K.prototype,`inputElement`,void 0),G([Ke()],K.prototype,`focused`,void 0),G([Ke()],K.prototype,`touched`,void 0),K=G([w(`lit-material-text-field`)],K);var St=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
    --notch-color: var(--md-sys-color-surface, #fffbfe);
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .textarea-field {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-width: 210px;
  }

  .container {
    position: relative;
    box-sizing: border-box;
    width: 100%;
    color: var(--md-sys-color-on-surface, #1c1b1f);
    cursor: text;
  }

  /* Unlike lit-material-text-field's single-line input, a multi-row box has
     nowhere sensible to vertically center a resting label — it always sits
     near the top, at roughly where the first line of typed text would be,
     and floating just shrinks/recolors it rather than moving it far. */
  .label {
    position: absolute;
    inset-inline-start: 16px;
    top: 16px;
    max-width: calc(100% - 32px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    pointer-events: none;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    font-family: var(--md-sys-typescale-body-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-large-size, 1rem);
    line-height: var(--md-sys-typescale-body-large-line-height, 1.5rem);
    transition:
      top 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      font-size 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .container.floated .label {
    top: 8px;
    font-size: var(--md-sys-typescale-label-medium-size, 0.75rem);
    line-height: var(--md-sys-typescale-label-medium-line-height, 1rem);
  }

  .container.focused .label {
    color: var(--md-sys-color-primary, #6750a4);
  }

  .textarea {
    display: block;
    box-sizing: border-box;
    width: 100%;
    margin: 0;
    padding: 16px;
    border: none;
    background: transparent;
    outline: none;
    color: inherit;
    font-family: var(--md-sys-typescale-body-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-large-size, 1rem);
    line-height: var(--md-sys-typescale-body-large-line-height, 1.5rem);
  }

  /* Once the label floats up out of the way, the first line of text can
     use the full box — otherwise it needs enough top padding to clear the
     resting label sitting where that first line would go. */
  .container.floated .textarea {
    padding-top: 24px;
  }

  /* Filled (default): solid surface container with a bottom active indicator. */
  :host([variant="filled"]) .container,
  :host(:not([variant])) .container {
    background-color: var(--md-sys-color-surface-container-highest, #e6e0e9);
    border-radius: var(--md-sys-shape-corner-extra-small, 4px) var(--md-sys-shape-corner-extra-small, 4px) 0 0;
  }

  :host([variant="filled"]) .container::after,
  :host(:not([variant])) .container::after {
    content: "";
    position: absolute;
    inset-inline: 0;
    bottom: 0;
    height: 1px;
    background-color: var(--md-sys-color-on-surface-variant, #49454f);
    transition: height 100ms linear, background-color 100ms linear;
  }

  :host([variant="filled"]) .container.focused::after,
  :host(:not([variant])) .container.focused::after {
    height: 2px;
    background-color: var(--md-sys-color-primary, #6750a4);
  }

  /* Outlined: transparent container with a full outline; the label notches it. */
  :host([variant="outlined"]) .container {
    background-color: transparent;
    border: 1px solid var(--md-sys-color-outline, #79747e);
    border-radius: var(--md-sys-shape-corner-extra-small, 4px);
    transition: border-color 100ms linear, border-width 100ms linear;
  }

  :host([variant="outlined"]) .container.focused {
    border-color: var(--md-sys-color-primary, #6750a4);
    border-width: 2px;
  }

  :host([variant="outlined"]) .container.floated .label {
    top: 0;
    transform: translateY(-50%);
    padding-inline: 4px;
    margin-inline-start: -4px;
    background-color: var(--notch-color);
  }

  /* Error state: red indicator + red label/supporting text. */
  :host([error]) .container.focused .label,
  :host([error]) .label {
    color: var(--md-sys-color-error, #b3261e);
  }

  :host([error]) .container::after {
    height: 2px;
    background-color: var(--md-sys-color-error, #b3261e);
  }

  :host([error][variant="outlined"]) .container,
  :host([error]:not([variant])) .container {
    border-color: var(--md-sys-color-error, #b3261e);
  }

  /* Disabled: dim everything; filled loses its indicator. */
  :host([disabled]) .container {
    opacity: 0.38;
  }

  :host([disabled][variant="filled"]) .container::after,
  :host([disabled]:not([variant])) .container::after {
    height: 1px;
    background-color: var(--md-sys-color-on-surface, #1c1b1f);
  }

  :host([disabled]) .textarea {
    cursor: default;
  }

  /* Supporting text + character counter row. */
  .supporting {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 16px 0;
    min-height: 1rem;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    font-family: var(--md-sys-typescale-label-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-medium-size, 0.75rem);
    line-height: var(--md-sys-typescale-label-medium-line-height, 1rem);
  }

  .supporting-text {
    flex: 1;
  }

  .counter {
    flex: none;
  }

  :host([error]) .supporting {
    color: var(--md-sys-color-error, #b3261e);
  }

  @media (prefers-reduced-motion: reduce) {
    .label,
    .container,
    .container::after {
      transition: none;
    }
  }
`,q=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Ct=0,J=class extends C{static{this.styles=St}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.rows=4,this.resize=`vertical`,this.autocomplete=``,this.name=``,this.focused=!1,this.touched=!1,this.labelId=`lit-material-textarea-label-${++Ct}`,this.supportingId=`lit-material-textarea-supporting-${Ct}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`minlength`)||e.has(`maxlength`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.textareaElement?!1:!this.textareaElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.textareaElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.textareaElement){let e=this.isError&&this.errorText||this.textareaElement.validationMessage;this.internals.setValidity(this.textareaElement.validity,e,this.textareaElement)}}render(){let e=this.floated,t=this.isError;return _`
      <div class="textarea-field" part="textarea-field">
        <div class="container ${e?`floated`:``} ${this.focused?`focused`:``}" part="container">
          ${this.label?_`<label class="label" part="label" id=${this.labelId} for="textarea">${this.label}</label>`:b}
          <textarea
            class="textarea"
            part="textarea"
            id="textarea"
            style="resize: ${this.resize}"
            rows=${this.rows}
            .value=${this.value}
            placeholder=${this.placeholder||b}
            ?disabled=${this.disabled}
            ?readonly=${this.readonly}
            ?required=${this.required}
            autocomplete=${this.autocomplete||b}
            minlength=${this.minlength??b}
            maxlength=${this.maxlength??b}
            name=${this.name||b}
            aria-labelledby=${this.label?this.labelId:b}
            aria-describedby=${this.supportingMessage?this.supportingId:b}
            aria-invalid=${t?`true`:b}
            @input=${this.handleInput}
            @change=${this.handleChange}
            @focus=${this.handleFocus}
            @blur=${this.handleBlur}
          ></textarea>
        </div>
        ${this.supportingMessage||this.maxlength!=null?_`
              <div class="supporting" part="supporting">
                <span class="supporting-text" id=${this.supportingId}> ${this.supportingMessage} </span>
                ${this.maxlength==null?b:_`<span class="counter">${this.value.length} / ${this.maxlength}</span>`}
              </div>
            `:b}
      </div>
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};q([T()],J.prototype,`variant`,void 0),q([T()],J.prototype,`value`,void 0),q([T()],J.prototype,`label`,void 0),q([T()],J.prototype,`placeholder`,void 0),q([T({attribute:`supporting-text`})],J.prototype,`supportingText`,void 0),q([T({attribute:`error-text`})],J.prototype,`errorText`,void 0),q([T({type:Boolean,reflect:!0})],J.prototype,`error`,void 0),q([T({type:Boolean,reflect:!0})],J.prototype,`disabled`,void 0),q([T({type:Boolean})],J.prototype,`required`,void 0),q([T({type:Boolean})],J.prototype,`readonly`,void 0),q([T({type:Number})],J.prototype,`minlength`,void 0),q([T({type:Number})],J.prototype,`maxlength`,void 0),q([T({type:Number})],J.prototype,`rows`,void 0),q([T()],J.prototype,`resize`,void 0),q([T()],J.prototype,`autocomplete`,void 0),q([T()],J.prototype,`name`,void 0),q([T({reflect:!0})],J.prototype,`form`,void 0),q([E(`.textarea`)],J.prototype,`textareaElement`,void 0),q([Ke()],J.prototype,`focused`,void 0),q([Ke()],J.prototype,`touched`,void 0),J=q([w(`lit-material-textarea`)],J);var wt=`rws-admin-token`,Tt=``;function Et(){try{return sessionStorage.getItem(wt)??Tt}catch{return Tt}}function Dt(e){Tt=e;try{e?sessionStorage.setItem(wt,e):sessionStorage.removeItem(wt)}catch{}}var Y=class extends Error{constructor(e,t){super(t),this.status=e}};async function Ot(e,{method:t=`GET`,body:n}={}){let r=await fetch(`/admin/api/v1${e}`,{method:t,headers:{Authorization:`Bearer ${Et()}`,...n===void 0?{}:{"Content-Type":`application/json`}},body:n===void 0?void 0:JSON.stringify(n),cache:`no-store`}),i=await r.text(),a=null;try{a=i?JSON.parse(i):null}catch{a=null}if(!r.ok)throw new Y(r.status,a?.error??`${r.status} ${r.statusText}`);return a}async function kt(e,t){let n=await fetch(`/admin/api/v1/logs/stream`,{headers:{Authorization:`Bearer ${Et()}`},signal:t,cache:`no-store`});if(!n.ok||!n.body)throw new Y(n.status,`${n.status} ${n.statusText}`);let r=n.body.pipeThrough(new TextDecoderStream).getReader(),i=``;for(;;){let{value:t,done:n}=await r.read();if(n)return;i+=t;let a;for(;(a=i.indexOf(`

`))>=0;){let t=i.slice(0,a);if(i=i.slice(a+2),t.split(`
`).some(e=>e.startsWith(`event:`)))continue;let n=t.split(`
`).filter(e=>e.startsWith(`data:`)).map(e=>e.slice(5).replace(/^ /,``)).join(`
`);n&&e(n)}}}var At={ATTRIBUTE:1,CHILD:2,PROPERTY:3,BOOLEAN_ATTRIBUTE:4,EVENT:5,ELEMENT:6},jt=e=>(...t)=>({_$litDirective$:e,values:t}),Mt=class{constructor(e){}get _$AU(){return this._$AM._$AU}_$AT(e,t,n){this._$Ct=e,this._$AM=t,this._$Ci=n}_$AS(e,t){return this.update(e,t)}update(e,t){return this.render(...t)}},Nt=class extends Mt{constructor(e){if(super(e),this.it=b,e.type!==At.CHILD)throw Error(this.constructor.directiveName+`() can only be used in child bindings`)}render(e){if(e===b||e==null)return this._t=void 0,this.it=e;if(e===y)return e;if(typeof e!=`string`)throw Error(this.constructor.directiveName+`() called with a non-string value`);if(e===this.it)return this._t;this.it=e;let t=[e];return t.raw=t,this._t={_$litType$:this.constructor.resultType,strings:t,values:[]}}};Nt.directiveName=`unsafeHTML`,Nt.resultType=1;var Pt=class extends Nt{};Pt.directiveName=`unsafeSVG`,Pt.resultType=2;var Ft=jt(Pt),It={gauge:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M207.06,72.67A111.24,111.24,0,0,0,128,40h-.4C66.07,40.21,16,91,16,153.13V176a16,16,0,0,0,16,16H224a16,16,0,0,0,16-16V152A111.25,111.25,0,0,0,207.06,72.67ZM224,176H119.71l54.76-75.3a8,8,0,0,0-12.94-9.42L99.92,176H32V153.13c0-3.08.15-6.12.43-9.13H56a8,8,0,0,0,0-16H35.27c10.32-38.86,44-68.24,84.73-71.66V80a8,8,0,0,0,16,0V56.33A96.14,96.14,0,0,1,221,128H200a8,8,0,0,0,0,16h23.67c.21,2.65.33,5.31.33,8Z"/></svg>`,"tree-structure":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M160,112h48a16,16,0,0,0,16-16V48a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16V64H128a24,24,0,0,0-24,24v32H72v-8A16,16,0,0,0,56,96H24A16,16,0,0,0,8,112v32a16,16,0,0,0,16,16H56a16,16,0,0,0,16-16v-8h32v32a24,24,0,0,0,24,24h16v16a16,16,0,0,0,16,16h48a16,16,0,0,0,16-16V160a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16v16H128a8,8,0,0,1-8-8V88a8,8,0,0,1,8-8h16V96A16,16,0,0,0,160,112ZM56,144H24V112H56v32Zm104,16h48v48H160Zm0-112h48V96H160Z"/></svg>`,certificate:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,136a8,8,0,0,1-8,8H72a8,8,0,0,1,0-16h48A8,8,0,0,1,128,136Zm-8-40H72a8,8,0,0,0,0,16h48a8,8,0,0,0,0-16Zm112,65.47V224A8,8,0,0,1,220,231l-24-13.74L172,231A8,8,0,0,1,160,224V200H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216a16,16,0,0,1,16,16V86.53a51.88,51.88,0,0,1,0,74.94ZM160,184V161.47A52,52,0,0,1,216,76V56H40V184Zm56-12a51.88,51.88,0,0,1-40,0v38.22l16-9.16a8,8,0,0,1,7.94,0l16,9.16Zm16-48a36,36,0,1,0-36,36A36,36,0,0,0,232,124Z"/></svg>`,"sliders-horizontal":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M40,88H73a32,32,0,0,0,62,0h81a8,8,0,0,0,0-16H135a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16Zm64-24A16,16,0,1,1,88,80,16,16,0,0,1,104,64ZM216,168H199a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16h97a32,32,0,0,0,62,0h17a8,8,0,0,0,0-16Zm-48,24a16,16,0,1,1,16-16A16,16,0,0,1,168,192Z"/></svg>`,"list-bullets":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M80,64a8,8,0,0,1,8-8H216a8,8,0,0,1,0,16H88A8,8,0,0,1,80,64Zm136,56H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16Zm0,64H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16ZM44,52A12,12,0,1,0,56,64,12,12,0,0,0,44,52Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,116Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,180Z"/></svg>`,"terminal-window":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,128a8,8,0,0,1-3,6.25l-40,32a8,8,0,1,1-10-12.5L107.19,128,75,102.25a8,8,0,1,1,10-12.5l40,32A8,8,0,0,1,128,128Zm48,24H136a8,8,0,0,0,0,16h40a8,8,0,0,0,0-16Zm56-96V200a16,16,0,0,1-16,16H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216A16,16,0,0,1,232,56ZM216,200V56H40V200H216Z"/></svg>`,"sign-out":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M120,216a8,8,0,0,1-8,8H48a8,8,0,0,1-8-8V40a8,8,0,0,1,8-8h64a8,8,0,0,1,0,16H56V208h56A8,8,0,0,1,120,216Zm109.66-93.66-40-40a8,8,0,0,0-11.32,11.32L204.69,120H112a8,8,0,0,0,0,16h92.69l-26.35,26.34a8,8,0,0,0,11.32,11.32l40-40A8,8,0,0,0,229.66,122.34Z"/></svg>`,"check-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M173.66,98.34a8,8,0,0,1,0,11.32l-56,56a8,8,0,0,1-11.32,0l-24-24a8,8,0,0,1,11.32-11.32L112,148.69l50.34-50.35A8,8,0,0,1,173.66,98.34ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,"warning-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24Zm0,192a88,88,0,1,1,88-88A88.1,88.1,0,0,1,128,216Zm-8-80V80a8,8,0,0,1,16,0v56a8,8,0,0,1-16,0Zm20,36a12,12,0,1,1-12-12A12,12,0,0,1,140,172Z"/></svg>`,"x-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M165.66,101.66,139.31,128l26.35,26.34a8,8,0,0,1-11.32,11.32L128,139.31l-26.34,26.35a8,8,0,0,1-11.32-11.32L116.69,128,90.34,101.66a8,8,0,0,1,11.32-11.32L128,116.69l26.34-26.35a8,8,0,0,1,11.32,11.32ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,pause:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M200,32H160a16,16,0,0,0-16,16V208a16,16,0,0,0,16,16h40a16,16,0,0,0,16-16V48A16,16,0,0,0,200,32Zm0,176H160V48h40ZM96,32H56A16,16,0,0,0,40,48V208a16,16,0,0,0,16,16H96a16,16,0,0,0,16-16V48A16,16,0,0,0,96,32Zm0,176H56V48H96Z"/></svg>`,play:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M232.4,114.49,88.32,26.35a16,16,0,0,0-16.2-.3A15.86,15.86,0,0,0,64,39.87V216.13A15.94,15.94,0,0,0,80,232a16.07,16.07,0,0,0,8.36-2.35L232.4,141.51a15.81,15.81,0,0,0,0-27ZM80,215.94V40l143.83,88Z"/></svg>`,"arrow-clockwise":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M240,56v48a8,8,0,0,1-8,8H184a8,8,0,0,1,0-16H211.4L184.81,71.64l-.25-.24a80,80,0,1,0-1.67,114.78,8,8,0,0,1,11,11.63A95.44,95.44,0,0,1,128,224h-1.32A96,96,0,1,1,195.75,60L224,85.8V56a8,8,0,1,1,16,0Z"/></svg>`,trash:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M216,48H176V40a24,24,0,0,0-24-24H104A24,24,0,0,0,80,40v8H40a8,8,0,0,0,0,16h8V208a16,16,0,0,0,16,16H192a16,16,0,0,0,16-16V64h8a8,8,0,0,0,0-16ZM96,40a8,8,0,0,1,8-8h48a8,8,0,0,1,8,8v8H96Zm96,168H64V64H192ZM112,104v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Zm48,0v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Z"/></svg>`,plus:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M224,128a8,8,0,0,1-8,8H136v80a8,8,0,0,1-16,0V136H40a8,8,0,0,1,0-16h80V40a8,8,0,0,1,16,0v80h80A8,8,0,0,1,224,128Z"/></svg>`,"lock-key":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,112a28,28,0,0,0-8,54.83V184a8,8,0,0,0,16,0V166.83A28,28,0,0,0,128,112Zm0,40a12,12,0,1,1,12-12A12,12,0,0,1,128,152Zm80-72H176V56a48,48,0,0,0-96,0V80H48A16,16,0,0,0,32,96V208a16,16,0,0,0,16,16H208a16,16,0,0,0,16-16V96A16,16,0,0,0,208,80ZM96,56a32,32,0,0,1,64,0V80H96ZM208,208H48V96H208V208Z"/></svg>`};function X(e){return Ft((It[e]??``).replace(`<svg `,`<svg aria-hidden="true" focusable="false" `))}var Lt=640,Rt=180,zt=44,Bt=8;function Vt(e){if(e<=1)return 1;let t=10**Math.floor(Math.log10(e));for(let n of[1,2,2.5,5,10])if(n*t>=e)return n*t;return 10*t}function Z(e){return e>=100?e.toFixed(0):e>=10?e.toFixed(1):e.toFixed(2)}function Ht(e){let t=Vt(Math.max(...e.map(e=>e.rate))),n=e[0].at,r=e[e.length-1].at,i=e=>zt+(e-n)/Math.max(1,r-n)*592,a=e=>Bt+(1-e/t)*150,o=e.map(e=>`${i(e.at).toFixed(1)},${a(e.rate).toFixed(1)}`).join(` `),s=[0,t/2,t],c=e[e.length-1],l=Math.max(...e.map(e=>e.rate)),u=t=>{let n=t.currentTarget.getBoundingClientRect(),r=(t.clientX-n.left)/n.width*Lt,a=e[0];for(let t of e)Math.abs(i(t.at)-r)<Math.abs(i(a.at)-r)&&(a=t);d(t.currentTarget.closest(`.chart`),a)},d=(e,t)=>{let n=e.querySelector(`.hover`),r=e.querySelector(`.tip`);n.setAttribute(`transform`,`translate(${i(t.at)},0)`),n.querySelector(`.dot`).setAttribute(`cy`,a(t.rate)),n.style.display=``,r.hidden=!1,r.style.left=`${i(t.at)/Lt*100}%`,r.textContent=`${Z(t.rate)} req/s · ${new Date(t.at).toLocaleTimeString()}`},ee=e=>{let t=e.currentTarget.closest(`.chart`);t.querySelector(`.hover`).style.display=`none`,t.querySelector(`.tip`).hidden=!0};return _`
    <div class="chart">
      <svg viewBox="0 0 ${Lt} ${Rt}" preserveAspectRatio="none" role="img" tabindex="0"
        aria-label="Requests per second: now ${Z(c.rate)}, peak ${Z(l)} over the last ${Math.round((r-n)/1e3)} seconds"
        @pointermove=${u} @pointerleave=${ee} @focus=${e=>d(e.currentTarget.closest(`.chart`),c)} @blur=${ee}>
        ${s.map(e=>v`
          <line class="grid-line" x1=${zt} x2=${Lt} y1=${a(e)} y2=${a(e)}></line>
          <text class="axis-text" x=${36} y=${a(e)+4} text-anchor="end">${Z(e)}</text>`)}
        <polyline class="series" points=${o} vector-effect="non-scaling-stroke"></polyline>
        <g class="hover" style="display:none">
          <line class="crosshair" x1="0" x2="0" y1=${Bt} y2=${158} vector-effect="non-scaling-stroke"></line>
          <circle class="dot" r="4" cx="0" cy="0" vector-effect="non-scaling-stroke"></circle>
        </g>
      </svg>
      <div class="tip" hidden></div>
      <p class="label" style="margin: var(--space-75) 0 0">Now ${Z(c.rate)} req/s, peak ${Z(l)}.</p>
    </div>`}var Q=[{id:`overview`,label:`Overview`,icon:`gauge`},{id:`upstreams`,label:`Upstreams`,icon:`tree-structure`},{id:`certificates`,label:`Certificates`,icon:`certificate`},{id:`controls`,label:`Controls`,icon:`sliders-horizontal`},{id:`config`,label:`Config`,icon:`list-bullets`},{id:`logs`,label:`Logs`,icon:`terminal-window`}];function Ut(){let e=location.hash.replace(/^#\/?/,``);return Q.some(t=>t.id===e)?e:`overview`}function Wt(e){let t=Math.floor(e/86400),n=Math.floor(e%86400/3600),r=Math.floor(e%3600/60);return t?`${t}d ${n}h`:n?`${n}h ${r}m`:`${r}m ${e%60}s`}function $(e,t,n){return _`<span class="status ${e}"><span class="icon">${X(n)}</span>${t}</span>`}var Gt=class extends C{static properties={authed:{state:!0},view:{state:!0},loginError:{state:!0},data:{state:!0},error:{state:!0},samples:{state:!0},lines:{state:!0},paused:{state:!0},filter:{state:!0},busy:{state:!0}};constructor(){super(),this.authed=!1,this.view=Ut(),this.data={},this.samples=[],this.lines=[],this.paused=!1,this.filter=``,this.busy=``,this.lastTotal=null}createRenderRoot(){return this}connectedCallback(){super.connectedCallback(),this.onHash=()=>this.show(Ut()),addEventListener(`hashchange`,this.onHash),Et()&&this.check()}disconnectedCallback(){super.disconnectedCallback(),removeEventListener(`hashchange`,this.onHash),this.stop()}async check(){try{let e=await Ot(`/status`);this.data={...this.data,status:e},this.authed=!0,this.loginError=``,this.show(this.view)}catch(e){this.authed=!1,this.loginError=e instanceof Y&&e.status===401?`That token isn't the admin token. Check RWS_ADMIN_TOKEN (or your token_env) on the server.`:`The admin API didn't answer: ${e.message}`,e instanceof Y&&e.status===401&&Dt(``)}}login(e){e.preventDefault();let t=(this.querySelector(`#token`)?.value??``).trim();t?(Dt(t),this.check()):this.loginError=`Paste the admin token.`}logout(){Dt(``),this.stop(),this.authed=!1,this.data={}}async call(e,t){try{let n=await Ot(e,t);return this.error=``,n}catch(e){throw e instanceof Y&&e.status===401?(this.logout(),this.loginError=`The server no longer accepts this token. Sign in again.`):this.error=e.message,e}}toast(e){let t=this.querySelector(`#toast`);t&&(t.textContent=e,t.show?.())}show(e){this.stop(),this.view=e,location.hash!==`#/${e}`&&history.replaceState(null,``,`#/${e}`),this.error=``;let t={overview:2e3,upstreams:3e3,certificates:3e4,controls:5e3}[e];this.load(e),t&&(this.timer=setInterval(()=>this.load(e),t)),e===`logs`&&this.follow(),queueMicrotask(()=>this.querySelector(`main`)?.focus({preventScroll:!0}))}stop(){clearInterval(this.timer),this.timer=void 0,this.abort?.abort(),this.abort=void 0}async load(e){let t=async(e,t)=>{try{let n=await this.call(t);this.data={...this.data,[e]:n}}catch{}};if(e===`overview`){await Promise.all([t(`status`,`/status`),t(`metrics`,`/metrics`)]);let e=this.data.metrics?.requests_total;if(typeof e==`number`){let t=Date.now();if(this.lastTotal){let n=Math.max(0,(e-this.lastTotal.total)/((t-this.lastTotal.at)/1e3));this.samples=[...this.samples,{at:t,rate:n}].slice(-60)}this.lastTotal={total:e,at:t}}}else e===`upstreams`?await t(`upstreams`,`/upstreams`):e===`certificates`?await t(`certificates`,`/certificates`):e===`controls`?await Promise.all([t(`maintenance`,`/maintenance`),t(`blocklist`,`/blocklist`),t(`flags`,`/flags`),t(`rateLimit`,`/rate-limit`),this.loadCache()]):e===`config`&&await t(`config`,`/config`)}async loadCache(){try{let e=await Ot(`/cache`);this.data={...this.data,cache:e}}catch(e){this.data={...this.data,cache:e instanceof Y&&e.status===404?null:this.data.cache}}}async follow(){this.abort=new AbortController;try{let e=await this.call(`/logs?n=200`);this.lines=e.map(e=>({text:e})),await kt(e=>{this.paused||(this.lines=[...this.lines,{text:e}].slice(-1e3),this.updateComplete.then(()=>{let e=this.querySelector(`.log`);e&&e.scrollHeight-e.scrollTop-e.clientHeight<80&&(e.scrollTop=e.scrollHeight)}))},this.abort.signal)}catch(e){e.name!==`AbortError`&&!this.error&&(this.error=`The live log stopped: ${e.message}`)}}async act(e,t,n,r){this.busy=e;try{let e=await this.call(t,n);r&&this.toast(r(e)),await this.load(this.view)}catch{}finally{this.busy=``}}drain(e,t,n){return this.act(`drain:${t}`,`/upstreams/${encodeURIComponent(e)}/drain`,{method:`PUT`,body:{backend:t,drained:n}},()=>n?`${t} drained: it gets no new requests`:`${t} is back in rotation`)}reload(){return this.act(`reload`,`/reload`,{method:`POST`},e=>e.applied?`Configuration reloaded`:`Reload kept the running configuration: ${e.errors.join(`; `)}`)}block(e){e.preventDefault();let t=this.querySelector(`#block-ip`),n=(t?.value??``).trim();n&&this.act(`block`,`/blocklist`,{method:`POST`,body:{ip:n}},e=>`${e.blocked} blocked`).then(()=>{!this.error&&t&&(t.value=``)})}render(){if(!this.authed)return this.renderLogin();let e=this.data.status;return _`
      <div class="shell">
        <header class="top">
          <span class="brand"><span class="brand-mark" aria-hidden="true"></span>rws admin</span>
          ${e?e.ready?$(`ok`,`Ready`,`check-circle`):$(`bad`,`Not ready`,`x-circle`):b}
          <span class="spacer"></span>
          ${e?_`<span class="meta">${e.version} · ${e.listeners?.main?.address??``}</span>`:b}
          <lit-material-button variant="text" @click=${()=>this.logout()}><span slot="icon" class="icon">${X(`sign-out`)}</span>Sign out</lit-material-button>
        </header>
        <nav class="side" aria-label="Sections">
          <lit-material-navigation-drawer .selected=${Q.findIndex(e=>e.id===this.view)} @change=${e=>this.show(Q[e.target.selected].id)}>
            ${Q.map(e=>_`<lit-material-navigation-drawer-item ?selected=${e.id===this.view}><span slot="icon">${X(e.icon)}</span>${e.label}</lit-material-navigation-drawer-item>`)}
          </lit-material-navigation-drawer>
        </nav>
        <main id="main" tabindex="-1">
          ${this.error?_`<lit-material-alert variant="error" class="section-alert"><span slot="title">Something failed</span>${this.error}</lit-material-alert>`:b}
          ${this.renderView()}
        </main>
        <nav class="bottom-nav" aria-label="Sections">
          <lit-material-navigation-bar .selected=${Q.findIndex(e=>e.id===this.view)} @change=${e=>this.show(Q[e.target.selected].id)}>
            ${Q.map(e=>_`<lit-material-navigation-bar-item ?selected=${e.id===this.view}><span slot="icon">${X(e.icon)}</span>${e.label}</lit-material-navigation-bar-item>`)}
          </lit-material-navigation-bar>
        </nav>
      </div>
      <lit-material-snackbar id="toast"></lit-material-snackbar>
    `}renderLogin(){return _`
      <div class="login">
        <section class="login-card tile" aria-labelledby="login-title">
          <h1 id="login-title"><span class="icon">${X(`lock-key`)}</span>Sign in to rws admin</h1>
          <p>Paste the admin token: the value of <code>RWS_ADMIN_TOKEN</code> (or the variable your <code>[admin] token_env</code> names). It stays in this tab only.</p>
          <form @submit=${e=>this.login(e)} novalidate>
            <lit-material-text-field id="token" variant="outlined" label="Admin token" type="password" autocomplete="current-password" required
              @keydown=${e=>e.key===`Enter`&&this.login(e)}
              ?error=${!!this.loginError} error-text=${this.loginError??``}></lit-material-text-field>
            <lit-material-button type="submit" @click=${e=>this.login(e)}>Sign in</lit-material-button>
          </form>
        </section>
      </div>
    `}renderView(){switch(this.view){case`upstreams`:return this.renderUpstreams();case`certificates`:return this.renderCertificates();case`controls`:return this.renderControls();case`config`:return this.renderConfig();case`logs`:return this.renderLogs();default:return this.renderOverview()}}skeleton(e=3){return _`<div class="grid" aria-busy="true">${Array.from({length:e},()=>_`<div class="tile"><lit-material-skeleton width="40%"></lit-material-skeleton><lit-material-skeleton variant="rounded" height="32px"></lit-material-skeleton></div>`)}</div>`}renderOverview(){let e=this.data.status,t=this.data.metrics;if(!e||!t)return _`<h1>Overview</h1>${this.skeleton(4)}`;let n=t.requests_total?(t.errors_total/t.requests_total*100).toFixed(2):`0.00`;return _`
      <h1>Overview</h1>
      <p class="lede">Up ${Wt(e.uptime_secs)}, process ${e.pid}. Counters refresh every 2 seconds.</p>
      <div class="grid">
        <div class="tile"><div class="label">Requests served</div><div class="value">${t.requests_total.toLocaleString()}</div></div>
        <div class="tile"><div class="label">Errors</div><div class="value">${t.errors_total.toLocaleString()}</div><div class="label">${n}% of requests</div></div>
        <div class="tile"><div class="label">Open connections</div><div class="value">${t.active_connections}</div></div>
        <div class="tile"><div class="label">Queued for a worker</div><div class="value">${t.thread_pool_queued}</div></div>
      </div>
      <section class="section" aria-labelledby="rate-title">
        <h2 id="rate-title">Requests per second</h2>
        ${this.samples.length<2?_`<div class="empty"><strong>Collecting samples</strong>The chart fills in as this page stays open.</div>`:Ht(this.samples)}
      </section>
      <section class="section" aria-labelledby="listen-title">
        <h2 id="listen-title">Listeners</h2>
        <div class="grid">
          <div class="tile"><div class="label">Main</div><div class="value mono">${e.listeners.main.address}</div><div class="label">${e.listeners.main.tls?`HTTPS, HTTP/2, HTTP/3`:`Plain HTTP`}</div></div>
          <div class="tile"><div class="label">HTTP redirect</div><div class="value mono">${e.listeners.redirect??`off`}</div></div>
          <div class="tile"><div class="label">Admin</div><div class="value mono">${e.listeners.admin}</div></div>
        </div>
      </section>
      <section class="section" aria-labelledby="build-title">
        <h2 id="build-title">Build</h2>
        <p class="lede">Version ${e.version} with ${e.features.join(`, `)}.</p>
      </section>
    `}renderUpstreams(){let e=this.data.upstreams;return e?_`
      <h1>Upstreams</h1>
      <p class="lede">Drain a backend before deploying it: new requests go elsewhere, requests already on it finish, and its health checks keep running.</p>
      ${e.length===0?_`<div class="empty"><strong>No upstreams configured</strong>Upstreams come from <code>[[upstream]]</code> sections in <code>rws.config.toml</code>. Add one, then reload from Controls.</div>`:e.map(e=>_`
          <section class="section" aria-labelledby="up-${e.name}">
            <h2 id="up-${e.name}">${e.name}</h2>
            <div class="table-wrap"><table>
              <thead><tr><th scope="col">Backend</th><th scope="col" class="num">Weight</th><th scope="col">Health</th><th scope="col">Breaker</th><th scope="col">In rotation</th></tr></thead>
              <tbody>${e.backends.map(t=>_`<tr>
                <td class="mono">${t.url}</td>
                <td class="num">${t.weight}</td>
                <td>${t.healthy?$(`ok`,`Healthy`,`check-circle`):$(`bad`,`Failing checks`,`x-circle`)}</td>
                <td>${t.breaker===null?$(`muted`,`None`,`list-bullets`):t.breaker===`closed`?$(`ok`,`Closed`,`check-circle`):t.breaker===`half_open`?$(`warn`,`Probing`,`warning-circle`):$(`bad`,`Open`,`x-circle`)}</td>
                <td><div class="row">
                  <lit-material-switch aria-label="${t.url} in rotation" .checked=${!t.drained} ?disabled=${this.busy===`drain:${t.url}`}
                    @change=${n=>this.drain(e.name,t.url,!n.target.checked)}></lit-material-switch>
                  ${t.drained?$(`warn`,`Drained`,`pause`):b}
                </div></td>
              </tr>`)}</tbody>
            </table></div>
          </section>`)}
    `:_`<h1>Upstreams</h1>${this.skeleton(2)}`}renderCertificates(){let e=this.data.certificates;if(!e)return _`<h1>Certificates</h1>${this.skeleton(2)}`;let t=e.acme;return _`
      <h1>Certificates</h1>
      <p class="lede">Every certificate this server presents, soonest expiry first.</p>
      ${e.certificates.length===0?_`<div class="empty"><strong>No TLS certificates</strong>This server speaks plain HTTP. Set <code>tls_cert_file</code> and <code>tls_key_file</code>, or <code>[acme] domains</code>, to serve HTTPS.</div>`:_`<div class="table-wrap"><table>
            <thead><tr><th scope="col">Name</th><th scope="col">Expires</th><th scope="col" class="num">Days left</th><th scope="col">Source</th></tr></thead>
            <tbody>${e.certificates.map(e=>_`<tr>
              <td class="mono">${e.name}</td>
              <td>${e.expires}</td>
              <td class="num">${e.days_left<7?$(`bad`,String(e.days_left),`warning-circle`):e.days_left<21?$(`warn`,String(e.days_left),`warning-circle`):e.days_left}</td>
              <td>${e.source===`on_demand`?`On demand`:`Configured`}</td>
            </tr>`)}</tbody>
          </table></div>`}
      <section class="section" aria-labelledby="acme-title">
        <h2 id="acme-title">ACME</h2>
        ${t.running?_`<p class="lede">Renewal runs automatically. ${t.last_check?_`Last check ${new Date(t.last_check.at*1e3).toLocaleString()}: ${t.last_check.outcome}.`:`No check has finished yet.`}</p>
              <lit-material-button variant="outlined" ?disabled=${this.busy===`renew`}
                @click=${()=>this.act(`renew`,`/certificates/renew`,{method:`POST`},()=>`Renewal requested: the new certificate is swapped in when it's issued`)}>
                <span slot="icon" class="icon">${X(`arrow-clockwise`)}</span>Renew now</lit-material-button>`:_`<div class="empty"><strong>ACME is off</strong>Set <code>[acme] domains</code> to have rws obtain and renew certificates itself.</div>`}
      </section>
    `}renderControls(){let e=this.data;return!e.maintenance||!e.blocklist||!e.flags?_`<h1>Controls</h1>${this.skeleton(3)}`:_`
      <h1>Controls</h1>
      <p class="lede">Changes here apply at once and last until the server restarts.</p>
      <div class="grid">
        <div class="tile">
          <div class="label">Maintenance mode</div>
          <div class="row" style="margin-top: var(--space-75)">
            <lit-material-switch aria-label="Maintenance mode" .checked=${e.maintenance.active} ?disabled=${this.busy===`maintenance`}
              @change=${e=>this.act(`maintenance`,`/maintenance`,{method:`PUT`,body:{active:e.target.checked}},e=>e.active?`Maintenance on: visitors get 503`:`Maintenance off`)}></lit-material-switch>
            ${e.maintenance.active?$(`warn`,`On: 503 for visitors`,`warning-circle`):$(`ok`,`Off`,`check-circle`)}
          </div>
        </div>
        <div class="tile">
          <div class="label">Configuration</div>
          <p class="label" style="margin: var(--space-50) 0 var(--space-100)">Re-read rws.config.toml and the certificates.</p>
          <lit-material-button variant="tonal" ?disabled=${this.busy===`reload`} @click=${()=>this.reload()}><span slot="icon" class="icon">${X(`arrow-clockwise`)}</span>Reload</lit-material-button>
        </div>
        <div class="tile">
          <div class="label">Response cache</div>
          ${e.cache?_`<div class="value">${e.cache.size}</div><div class="label">${e.cache.hits} hits, ${e.cache.misses} misses</div>
                <lit-material-button variant="text" ?disabled=${this.busy===`cache`} @click=${()=>this.act(`cache`,`/cache`,{method:`DELETE`},()=>`Cache cleared`)}><span slot="icon" class="icon">${X(`trash`)}</span>Clear</lit-material-button>`:_`<div class="value mono">None</div><div class="label">This server has no response cache.</div>`}
        </div>
        <div class="tile">
          <div class="label">Global rate limit</div>
          <div class="value mono">${e.rateLimit?.configured?`${e.rateLimit.max_requests} per ${e.rateLimit.window_secs}s`:`Off`}</div>
        </div>
      </div>

      <section class="section" aria-labelledby="block-title">
        <h2 id="block-title">Blocked addresses</h2>
        <form class="row" @submit=${e=>this.block(e)} novalidate>
          <lit-material-text-field id="block-ip" variant="outlined" label="IP address" supporting-text="IPv4 or IPv6, e.g. 203.0.113.7"
            @keydown=${e=>e.key===`Enter`&&this.block(e)}></lit-material-text-field>
          <lit-material-button type="submit" ?disabled=${this.busy===`block`} @click=${e=>this.block(e)}><span slot="icon" class="icon">${X(`plus`)}</span>Block</lit-material-button>
        </form>
        ${e.blocklist.length===0?_`<p class="lede" style="margin-top: var(--space-200)">Nobody is blocked.</p>`:_`<div class="table-wrap section" style="margin-top: var(--space-200)"><table>
              <thead><tr><th scope="col">Address</th><th scope="col"><span class="visually-hidden">Action</span></th></tr></thead>
              <tbody>${e.blocklist.map(e=>_`<tr><td class="mono">${e}</td><td>
                <lit-material-button variant="text" @click=${()=>this.act(`unblock:${e}`,`/blocklist/${encodeURIComponent(e)}`,{method:`DELETE`},()=>`${e} unblocked`)}>Unblock</lit-material-button>
              </td></tr>`)}</tbody>
            </table></div>`}
      </section>

      <section class="section" aria-labelledby="flags-title">
        <h2 id="flags-title">Feature flags</h2>
        ${e.flags.length===0?_`<div class="empty"><strong>No feature flags</strong>Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</div>`:_`<div class="table-wrap"><table>
              <thead><tr><th scope="col">Flag</th><th scope="col">Enabled</th></tr></thead>
              <tbody>${e.flags.map(e=>_`<tr><td class="mono">${e.name}</td><td>
                <lit-material-switch aria-label="${e.name}" .checked=${e.enabled}
                  @change=${t=>this.act(`flag:${e.name}`,`/flags/${encodeURIComponent(e.name)}`,{method:`PUT`,body:{enabled:t.target.checked}},e=>`${e.name} ${e.enabled?`on`:`off`}`)}></lit-material-switch>
              </td></tr>`)}</tbody>
            </table></div>`}
      </section>
    `}renderConfig(){let e=this.data.config;if(!e)return _`<h1>Config</h1>${this.skeleton(2)}`;let t=this.filter.toLowerCase(),n=Object.entries(e).filter(([e,n])=>!t||e.toLowerCase().includes(t)||n.toLowerCase().includes(t));return _`
      <h1>Config</h1>
      <p class="lede">Every setting in effect. Secrets read (hidden). Change them in rws.config.toml or the environment, then reload from Controls.</p>
      <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${e=>this.filter=e.target.value}></lit-material-text-field>
      ${n.length===0?_`<div class="empty"><strong>No setting matches "${this.filter}"</strong>Clear the filter to see all ${Object.keys(e).length}.</div>`:_`<div class="table-wrap"><table>
            <thead><tr><th scope="col">Setting</th><th scope="col">Value</th></tr></thead>
            <tbody>${n.map(([e,t])=>_`<tr><td class="mono">${e}</td><td class="mono">${t===``?_`<span class="status muted">empty</span>`:t}</td></tr>`)}</tbody>
          </table></div>`}
    `}renderLogs(){return _`
      <h1>Logs</h1>
      <div class="row" style="margin-bottom: var(--space-200)">
        <p class="lede" style="margin: 0; flex: 1">The access log as it's written, newest at the bottom.</p>
        <lit-material-button variant="outlined" @click=${()=>this.paused=!this.paused}>
          <span slot="icon" class="icon">${X(this.paused?`play`:`pause`)}</span>${this.paused?`Resume`:`Pause`}</lit-material-button>
      </div>
      ${this.lines.length===0?_`<div class="empty"><strong>Waiting for requests</strong>Lines appear as the server handles requests. With <code>log_format = "off"</code> there are none.</div>`:_`<div class="log" role="log" aria-live="off">${this.lines.map(e=>_`<div class=${/"status":[45]\d\d/.test(e.text)?`err`:``}>${e.text}</div>`)}</div>`}
    `}};customElements.define(`rws-admin`,Gt);