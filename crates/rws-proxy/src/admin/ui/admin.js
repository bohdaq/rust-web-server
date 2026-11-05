var e=globalThis,t=e.ShadowRoot&&(e.ShadyCSS===void 0||e.ShadyCSS.nativeShadow)&&`adoptedStyleSheets`in Document.prototype&&`replace`in CSSStyleSheet.prototype,n=Symbol(),r=new WeakMap,i=class{constructor(e,t,r){if(this._$cssResult$=!0,r!==n)throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");this.cssText=e,this.t=t}get styleSheet(){let e=this.o,n=this.t;if(t&&e===void 0){let t=n!==void 0&&n.length===1;t&&(e=r.get(n)),e===void 0&&((this.o=e=new CSSStyleSheet).replaceSync(this.cssText),t&&r.set(n,e))}return e}toString(){return this.cssText}},a=e=>new i(typeof e==`string`?e:e+``,void 0,n),o=(e,...t)=>new i(e.length===1?e[0]:t.reduce((t,n,r)=>t+(e=>{if(!0===e._$cssResult$)return e.cssText;if(typeof e==`number`)return e;throw Error(`Value passed to 'css' function must be a 'css' function result: `+e+`. Use 'unsafeCSS' to pass non-literal values, but take care to ensure page security.`)})(n)+e[r+1],e[0]),e,n),s=(n,r)=>{if(t)n.adoptedStyleSheets=r.map(e=>e instanceof CSSStyleSheet?e:e.styleSheet);else for(let t of r){let r=document.createElement(`style`),i=e.litNonce;i!==void 0&&r.setAttribute(`nonce`,i),r.textContent=t.cssText,n.appendChild(r)}},c=t?e=>e:e=>e instanceof CSSStyleSheet?(e=>{let t=``;for(let n of e.cssRules)t+=n.cssText;return a(t)})(e):e,{is:l,defineProperty:u,getOwnPropertyDescriptor:d,getOwnPropertyNames:f,getOwnPropertySymbols:p,getPrototypeOf:m}=Object,h=globalThis,ee=h.trustedTypes,te=ee?ee.emptyScript:``,ne=h.reactiveElementPolyfillSupport,g=(e,t)=>e,_={toAttribute(e,t){switch(t){case Boolean:e=e?te:null;break;case Object:case Array:e=e==null?e:JSON.stringify(e)}return e},fromAttribute(e,t){let n=e;switch(t){case Boolean:n=e!==null;break;case Number:n=e===null?null:Number(e);break;case Object:case Array:try{n=JSON.parse(e)}catch{n=null}}return n}},v=(e,t)=>!l(e,t),re={attribute:!0,type:String,converter:_,reflect:!1,useDefault:!1,hasChanged:v};Symbol.metadata??=Symbol(`metadata`),h.litPropertyMetadata??=new WeakMap;var ie=class extends HTMLElement{static addInitializer(e){this._$Ei(),(this.l??=[]).push(e)}static get observedAttributes(){return this.finalize(),this._$Eh&&[...this._$Eh.keys()]}static createProperty(e,t=re){if(t.state&&(t.attribute=!1),this._$Ei(),this.prototype.hasOwnProperty(e)&&((t=Object.create(t)).wrapped=!0),this.elementProperties.set(e,t),!t.noAccessor){let n=Symbol(),r=this.getPropertyDescriptor(e,n,t);r!==void 0&&u(this.prototype,e,r)}}static getPropertyDescriptor(e,t,n){let{get:r,set:i}=d(this.prototype,e)??{get(){return this[t]},set(e){this[t]=e}};return{get:r,set(t){let a=r?.call(this);i?.call(this,t),this.requestUpdate(e,a,n)},configurable:!0,enumerable:!0}}static getPropertyOptions(e){return this.elementProperties.get(e)??re}static _$Ei(){if(this.hasOwnProperty(g(`elementProperties`)))return;let e=m(this);e.finalize(),e.l!==void 0&&(this.l=[...e.l]),this.elementProperties=new Map(e.elementProperties)}static finalize(){if(this.hasOwnProperty(g(`finalized`)))return;if(this.finalized=!0,this._$Ei(),this.hasOwnProperty(g(`properties`))){let e=this.properties,t=[...f(e),...p(e)];for(let n of t)this.createProperty(n,e[n])}let e=this[Symbol.metadata];if(e!==null){let t=litPropertyMetadata.get(e);if(t!==void 0)for(let[e,n]of t)this.elementProperties.set(e,n)}this._$Eh=new Map;for(let[e,t]of this.elementProperties){let n=this._$Eu(e,t);n!==void 0&&this._$Eh.set(n,e)}this.elementStyles=this.finalizeStyles(this.styles)}static finalizeStyles(e){let t=[];if(Array.isArray(e)){let n=new Set(e.flat(1/0).reverse());for(let e of n)t.unshift(c(e))}else e!==void 0&&t.push(c(e));return t}static _$Eu(e,t){let n=t.attribute;return!1===n?void 0:typeof n==`string`?n:typeof e==`string`?e.toLowerCase():void 0}constructor(){super(),this._$Ep=void 0,this.isUpdatePending=!1,this.hasUpdated=!1,this._$Em=null,this._$Ev()}_$Ev(){this._$ES=new Promise(e=>this.enableUpdating=e),this._$AL=new Map,this._$E_(),this.requestUpdate(),this.constructor.l?.forEach(e=>e(this))}addController(e){(this._$EO??=new Set).add(e),this.renderRoot!==void 0&&this.isConnected&&e.hostConnected?.()}removeController(e){this._$EO?.delete(e)}_$E_(){let e=new Map,t=this.constructor.elementProperties;for(let n of t.keys())this.hasOwnProperty(n)&&(e.set(n,this[n]),delete this[n]);e.size>0&&(this._$Ep=e)}createRenderRoot(){let e=this.shadowRoot??this.attachShadow(this.constructor.shadowRootOptions);return s(e,this.constructor.elementStyles),e}connectedCallback(){this.renderRoot??=this.createRenderRoot(),this.enableUpdating(!0),this._$EO?.forEach(e=>e.hostConnected?.())}enableUpdating(e){}disconnectedCallback(){this._$EO?.forEach(e=>e.hostDisconnected?.())}attributeChangedCallback(e,t,n){this._$AK(e,n)}_$ET(e,t){let n=this.constructor.elementProperties.get(e),r=this.constructor._$Eu(e,n);if(r!==void 0&&!0===n.reflect){let i=(n.converter?.toAttribute===void 0?_:n.converter).toAttribute(t,n.type);this._$Em=e,i==null?this.removeAttribute(r):this.setAttribute(r,i),this._$Em=null}}_$AK(e,t){let n=this.constructor,r=n._$Eh.get(e);if(r!==void 0&&this._$Em!==r){let e=n.getPropertyOptions(r),i=typeof e.converter==`function`?{fromAttribute:e.converter}:e.converter?.fromAttribute===void 0?_:e.converter;this._$Em=r;let a=i.fromAttribute(t,e.type);this[r]=a??this._$Ej?.get(r)??a,this._$Em=null}}requestUpdate(e,t,n,r=!1,i){if(e!==void 0){let a=this.constructor;if(!1===r&&(i=this[e]),n??=a.getPropertyOptions(e),!((n.hasChanged??v)(i,t)||n.useDefault&&n.reflect&&i===this._$Ej?.get(e)&&!this.hasAttribute(a._$Eu(e,n))))return;this.C(e,t,n)}!1===this.isUpdatePending&&(this._$ES=this._$EP())}C(e,t,{useDefault:n,reflect:r,wrapped:i},a){n&&!(this._$Ej??=new Map).has(e)&&(this._$Ej.set(e,a??t??this[e]),!0!==i||a!==void 0)||(this._$AL.has(e)||(this.hasUpdated||n||(t=void 0),this._$AL.set(e,t)),!0===r&&this._$Em!==e&&(this._$Eq??=new Set).add(e))}async _$EP(){this.isUpdatePending=!0;try{await this._$ES}catch(e){Promise.reject(e)}let e=this.scheduleUpdate();return e!=null&&await e,!this.isUpdatePending}scheduleUpdate(){return this.performUpdate()}performUpdate(){if(!this.isUpdatePending)return;if(!this.hasUpdated){if(this.renderRoot??=this.createRenderRoot(),this._$Ep){for(let[e,t]of this._$Ep)this[e]=t;this._$Ep=void 0}let e=this.constructor.elementProperties;if(e.size>0)for(let[t,n]of e){let{wrapped:e}=n,r=this[t];!0!==e||this._$AL.has(t)||r===void 0||this.C(t,void 0,n,r)}}let e=!1,t=this._$AL;try{e=this.shouldUpdate(t),e?(this.willUpdate(t),this._$EO?.forEach(e=>e.hostUpdate?.()),this.update(t)):this._$EM()}catch(t){throw e=!1,this._$EM(),t}e&&this._$AE(t)}willUpdate(e){}_$AE(e){this._$EO?.forEach(e=>e.hostUpdated?.()),this.hasUpdated||(this.hasUpdated=!0,this.firstUpdated(e)),this.updated(e)}_$EM(){this._$AL=new Map,this.isUpdatePending=!1}get updateComplete(){return this.getUpdateComplete()}getUpdateComplete(){return this._$ES}shouldUpdate(e){return!0}update(e){this._$Eq&&=this._$Eq.forEach(e=>this._$ET(e,this[e])),this._$EM()}updated(e){}firstUpdated(e){}};ie.elementStyles=[],ie.shadowRootOptions={mode:`open`},ie[g(`elementProperties`)]=new Map,ie[g(`finalized`)]=new Map,ne?.({ReactiveElement:ie}),(h.reactiveElementVersions??=[]).push(`2.1.2`);var ae=globalThis,y=e=>e,oe=ae.trustedTypes,se=oe?oe.createPolicy(`lit-html`,{createHTML:e=>e}):void 0,ce=`$lit$`,b=`lit$${Math.random().toFixed(9).slice(2)}$`,le=`?`+b,ue=`<${le}>`,de=document,fe=()=>de.createComment(``),pe=e=>e===null||typeof e!=`object`&&typeof e!=`function`,me=Array.isArray,he=e=>me(e)||typeof e?.[Symbol.iterator]==`function`,ge=`[ 	
\f\r]`,_e=/<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g,ve=/-->/g,ye=/>/g,be=RegExp(`>|${ge}(?:([^\\s"'>=/]+)(${ge}*=${ge}*(?:[^ \t\n\f\r"'\`<>=]|("|')|))|$)`,`g`),xe=/'/g,Se=/"/g,Ce=/^(?:script|style|textarea|title)$/i,we=e=>(t,...n)=>({_$litType$:e,strings:t,values:n}),x=we(1),S=we(2),C=Symbol.for(`lit-noChange`),w=Symbol.for(`lit-nothing`),Te=new WeakMap,Ee=de.createTreeWalker(de,129);function De(e,t){if(!me(e)||!e.hasOwnProperty(`raw`))throw Error(`invalid template strings array`);return se===void 0?t:se.createHTML(t)}var Oe=(e,t)=>{let n=e.length-1,r=[],i,a=t===2?`<svg>`:t===3?`<math>`:``,o=_e;for(let t=0;t<n;t++){let n=e[t],s,c,l=-1,u=0;for(;u<n.length&&(o.lastIndex=u,c=o.exec(n),c!==null);)u=o.lastIndex,o===_e?c[1]===`!--`?o=ve:c[1]===void 0?c[2]===void 0?c[3]!==void 0&&(o=be):(Ce.test(c[2])&&(i=RegExp(`</`+c[2],`g`)),o=be):o=ye:o===be?c[0]===`>`?(o=i??_e,l=-1):c[1]===void 0?l=-2:(l=o.lastIndex-c[2].length,s=c[1],o=c[3]===void 0?be:c[3]===`"`?Se:xe):o===Se||o===xe?o=be:o===ve||o===ye?o=_e:(o=be,i=void 0);let d=o===be&&e[t+1].startsWith(`/>`)?` `:``;a+=o===_e?n+ue:l>=0?(r.push(s),n.slice(0,l)+ce+n.slice(l)+b+d):n+b+(l===-2?t:d)}return[De(e,a+(e[n]||`<?>`)+(t===2?`</svg>`:t===3?`</math>`:``)),r]},ke=class e{constructor({strings:t,_$litType$:n},r){let i;this.parts=[];let a=0,o=0,s=t.length-1,c=this.parts,[l,u]=Oe(t,n);if(this.el=e.createElement(l,r),Ee.currentNode=this.el.content,n===2||n===3){let e=this.el.content.firstChild;e.replaceWith(...e.childNodes)}for(;(i=Ee.nextNode())!==null&&c.length<s;){if(i.nodeType===1){if(i.hasAttributes())for(let e of i.getAttributeNames())if(e.endsWith(ce)){let t=u[o++],n=i.getAttribute(e).split(b),r=/([.?@])?(.*)/.exec(t);c.push({type:1,index:a,name:r[2],strings:n,ctor:r[1]===`.`?Pe:r[1]===`?`?Fe:r[1]===`@`?Ie:Ne}),i.removeAttribute(e)}else e.startsWith(b)&&(c.push({type:6,index:a}),i.removeAttribute(e));if(Ce.test(i.tagName)){let e=i.textContent.split(b),t=e.length-1;if(t>0){i.textContent=oe?oe.emptyScript:``;for(let n=0;n<t;n++)i.append(e[n],fe()),Ee.nextNode(),c.push({type:2,index:++a});i.append(e[t],fe())}}}else if(i.nodeType===8){if(i.data===le)c.push({type:2,index:a});else{let e=-1;for(;(e=i.data.indexOf(b,e+1))!==-1;)c.push({type:7,index:a}),e+=b.length-1}}a++}}static createElement(e,t){let n=de.createElement(`template`);return n.innerHTML=e,n}};function Ae(e,t,n=e,r){if(t===C)return t;let i=r===void 0?n._$Cl:n._$Co?.[r],a=pe(t)?void 0:t._$litDirective$;return i?.constructor!==a&&(i?._$AO?.(!1),a===void 0?i=void 0:(i=new a(e),i._$AT(e,n,r)),r===void 0?n._$Cl=i:(n._$Co??=[])[r]=i),i!==void 0&&(t=Ae(e,i._$AS(e,t.values),i,r)),t}var je=class{constructor(e,t){this._$AV=[],this._$AN=void 0,this._$AD=e,this._$AM=t}get parentNode(){return this._$AM.parentNode}get _$AU(){return this._$AM._$AU}u(e){let{el:{content:t},parts:n}=this._$AD,r=(e?.creationScope??de).importNode(t,!0);Ee.currentNode=r;let i=Ee.nextNode(),a=0,o=0,s=n[0];for(;s!==void 0;){if(a===s.index){let t;s.type===2?t=new Me(i,i.nextSibling,this,e):s.type===1?t=new s.ctor(i,s.name,s.strings,this,e):s.type===6&&(t=new Le(i,this,e)),this._$AV.push(t),s=n[++o]}a!==s?.index&&(i=Ee.nextNode(),a++)}return Ee.currentNode=de,r}p(e){let t=0;for(let n of this._$AV)n!==void 0&&(n.strings===void 0?n._$AI(e[t]):(n._$AI(e,n,t),t+=n.strings.length-2)),t++}},Me=class e{get _$AU(){return this._$AM?._$AU??this._$Cv}constructor(e,t,n,r){this.type=2,this._$AH=w,this._$AN=void 0,this._$AA=e,this._$AB=t,this._$AM=n,this.options=r,this._$Cv=r?.isConnected??!0}get parentNode(){let e=this._$AA.parentNode,t=this._$AM;return t!==void 0&&e?.nodeType===11&&(e=t.parentNode),e}get startNode(){return this._$AA}get endNode(){return this._$AB}_$AI(e,t=this){e=Ae(this,e,t),pe(e)?e===w||e==null||e===``?(this._$AH!==w&&this._$AR(),this._$AH=w):e!==this._$AH&&e!==C&&this._(e):e._$litType$===void 0?e.nodeType===void 0?he(e)?this.k(e):this._(e):this.T(e):this.$(e)}O(e){return this._$AA.parentNode.insertBefore(e,this._$AB)}T(e){this._$AH!==e&&(this._$AR(),this._$AH=this.O(e))}_(e){this._$AH!==w&&pe(this._$AH)?this._$AA.nextSibling.data=e:this.T(de.createTextNode(e)),this._$AH=e}$(e){let{values:t,_$litType$:n}=e,r=typeof n==`number`?this._$AC(e):(n.el===void 0&&(n.el=ke.createElement(De(n.h,n.h[0]),this.options)),n);if(this._$AH?._$AD===r)this._$AH.p(t);else{let e=new je(r,this),n=e.u(this.options);e.p(t),this.T(n),this._$AH=e}}_$AC(e){let t=Te.get(e.strings);return t===void 0&&Te.set(e.strings,t=new ke(e)),t}k(t){me(this._$AH)||(this._$AH=[],this._$AR());let n=this._$AH,r,i=0;for(let a of t)i===n.length?n.push(r=new e(this.O(fe()),this.O(fe()),this,this.options)):r=n[i],r._$AI(a),i++;i<n.length&&(this._$AR(r&&r._$AB.nextSibling,i),n.length=i)}_$AR(e=this._$AA.nextSibling,t){for(this._$AP?.(!1,!0,t);e!==this._$AB;){let t=y(e).nextSibling;y(e).remove(),e=t}}setConnected(e){this._$AM===void 0&&(this._$Cv=e,this._$AP?.(e))}},Ne=class{get tagName(){return this.element.tagName}get _$AU(){return this._$AM._$AU}constructor(e,t,n,r,i){this.type=1,this._$AH=w,this._$AN=void 0,this.element=e,this.name=t,this._$AM=r,this.options=i,n.length>2||n[0]!==``||n[1]!==``?(this._$AH=Array(n.length-1).fill(new String),this.strings=n):this._$AH=w}_$AI(e,t=this,n,r){let i=this.strings,a=!1;if(i===void 0)e=Ae(this,e,t,0),a=!pe(e)||e!==this._$AH&&e!==C,a&&(this._$AH=e);else{let r=e,o,s;for(e=i[0],o=0;o<i.length-1;o++)s=Ae(this,r[n+o],t,o),s===C&&(s=this._$AH[o]),a||=!pe(s)||s!==this._$AH[o],s===w?e=w:e!==w&&(e+=(s??``)+i[o+1]),this._$AH[o]=s}a&&!r&&this.j(e)}j(e){e===w?this.element.removeAttribute(this.name):this.element.setAttribute(this.name,e??``)}},Pe=class extends Ne{constructor(){super(...arguments),this.type=3}j(e){this.element[this.name]=e===w?void 0:e}},Fe=class extends Ne{constructor(){super(...arguments),this.type=4}j(e){this.element.toggleAttribute(this.name,!!e&&e!==w)}},Ie=class extends Ne{constructor(e,t,n,r,i){super(e,t,n,r,i),this.type=5}_$AI(e,t=this){if((e=Ae(this,e,t,0)??w)===C)return;let n=this._$AH,r=e===w&&n!==w||e.capture!==n.capture||e.once!==n.once||e.passive!==n.passive,i=e!==w&&(n===w||r);r&&this.element.removeEventListener(this.name,this,n),i&&this.element.addEventListener(this.name,this,e),this._$AH=e}handleEvent(e){typeof this._$AH==`function`?this._$AH.call(this.options?.host??this.element,e):this._$AH.handleEvent(e)}},Le=class{constructor(e,t,n){this.element=e,this.type=6,this._$AN=void 0,this._$AM=t,this.options=n}get _$AU(){return this._$AM._$AU}_$AI(e){Ae(this,e)}},Re={M:ce,P:b,A:le,C:1,L:Oe,R:je,D:he,V:Ae,I:Me,H:Ne,N:Fe,U:Ie,B:Pe,F:Le},ze=ae.litHtmlPolyfillSupport;ze?.(ke,Me),(ae.litHtmlVersions??=[]).push(`3.3.3`);var Be=(e,t,n)=>{let r=n?.renderBefore??t,i=r._$litPart$;if(i===void 0){let e=n?.renderBefore??null;r._$litPart$=i=new Me(t.insertBefore(fe(),e),e,void 0,n??{})}return i._$AI(e),i},Ve=globalThis,T=class extends ie{constructor(){super(...arguments),this.renderOptions={host:this},this._$Do=void 0}createRenderRoot(){let e=super.createRenderRoot();return this.renderOptions.renderBefore??=e.firstChild,e}update(e){let t=this.render();this.hasUpdated||(this.renderOptions.isConnected=this.isConnected),super.update(e),this._$Do=Be(t,this.renderRoot,this.renderOptions)}connectedCallback(){super.connectedCallback(),this._$Do?.setConnected(!0)}disconnectedCallback(){super.disconnectedCallback(),this._$Do?.setConnected(!1)}render(){return C}};T._$litElement$=!0,T.finalized=!0,Ve.litElementHydrateSupport?.({LitElement:T});var He=Ve.litElementPolyfillSupport;He?.({LitElement:T}),(Ve.litElementVersions??=[]).push(`4.2.2`);var E=e=>(t,n)=>{n===void 0?customElements.define(e,t):n.addInitializer(()=>{customElements.define(e,t)})},Ue={attribute:!0,type:String,converter:_,reflect:!1,hasChanged:v},We=(e=Ue,t,n)=>{let{kind:r,metadata:i}=n,a=globalThis.litPropertyMetadata.get(i);if(a===void 0&&globalThis.litPropertyMetadata.set(i,a=new Map),r===`setter`&&((e=Object.create(e)).wrapped=!0),a.set(n.name,e),r===`accessor`){let{name:r}=n;return{set(n){let i=t.get.call(this);t.set.call(this,n),this.requestUpdate(r,i,e,!0,n)},init(t){return t!==void 0&&this.C(r,void 0,e,t),t}}}if(r===`setter`){let{name:r}=n;return function(n){let i=this[r];t.call(this,n),this.requestUpdate(r,i,e,!0,n)}}throw Error(`Unsupported decorator location: `+r)};function D(e){return(t,n)=>typeof n==`object`?We(e,t,n):((e,t,n)=>{let r=t.hasOwnProperty(n);return t.constructor.createProperty(n,e),r?Object.getOwnPropertyDescriptor(t,n):void 0})(e,t,n)}function O(e){return D({...e,state:!0,attribute:!1})}var Ge=(e,t,n)=>(n.configurable=!0,n.enumerable=!0,Reflect.decorate&&typeof t!=`object`&&Object.defineProperty(e,t,n),n);function k(e,t){return(n,r,i)=>{let a=t=>t.renderRoot?.querySelector(e)??null;if(t){let{get:e,set:t}=typeof r==`object`?n:i??(()=>{let e=Symbol();return{get(){return this[e]},set(t){this[e]=t}}})();return Ge(n,r,{get(){let n=e.call(this);return n===void 0&&(n=a(this),(n!==null||this.hasUpdated)&&t.call(this,n)),n}})}return Ge(n,r,{get(){return a(this)}})}}var Ke=o`
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
    border-radius: var(--lit-material-alert-shape, var(--md-sys-shape-corner-small, 8px));
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
`,qe=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Je={info:S`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <line x1="12" y1="11" x2="12" y2="16" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="7.5" r="1.25" fill="currentColor"></circle>
    </svg>
  `,success:S`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <polyline points="7,12.5 10.5,16 17,8.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></polyline>
    </svg>
  `,warning:S`
    <svg viewBox="0 0 24 24">
      <polygon points="12,3 22,20 2,20" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"></polygon>
      <line x1="12" y1="9" x2="12" y2="14" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="17" r="1.15" fill="currentColor"></circle>
    </svg>
  `,error:S`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <line x1="12" y1="7" x2="12" y2="13" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="16.5" r="1.25" fill="currentColor"></circle>
    </svg>
  `},Ye=class extends T{constructor(){super(...arguments),this.variant=`info`,this.dismissible=!1,this.roleIsOwn=!1}static{this.styles=Ke}willUpdate(){(!this.hasAttribute(`role`)||this.roleIsOwn)&&(this.setAttribute(`role`,this.variant===`warning`||this.variant===`error`?`alert`:`status`),this.roleIsOwn=!0)}dismiss(){this.hidden=!0,this.dispatchEvent(new Event(`close`,{bubbles:!0}))}render(){return x`
      <div class="alert" part="alert">
        <span class="icon" part="icon" aria-hidden="true">
          <slot name="icon">${Je[this.variant]}</slot>
        </span>
        <div class="body" part="body">
          <slot name="title" class="title"></slot>
          <div class="description" part="description"><slot></slot></div>
          <slot name="action" class="actions"></slot>
        </div>
        ${this.dismissible?x`
              <button class="close" part="close" type="button" aria-label="Dismiss" @click=${this.dismiss}>
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
                  <line x1="6" y1="18" x2="18" y2="6" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
                </svg>
              </button>
            `:w}
      </div>
    `}};qe([D()],Ye.prototype,`variant`,void 0),qe([D({type:Boolean,reflect:!0})],Ye.prototype,`dismissible`,void 0),Ye=qe([E(`lit-material-alert`)],Ye);var Xe=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},Ze=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}},Qe=o`
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
    border-start-start-radius: var(--lit-material-button-shape-start, var(--lit-material-button-shape, var(--md-sys-shape-corner-full, 9999px)));
    border-end-start-radius: var(--lit-material-button-shape-start, var(--lit-material-button-shape, var(--md-sys-shape-corner-full, 9999px)));
    border-start-end-radius: var(--lit-material-button-shape-end, var(--lit-material-button-shape, var(--md-sys-shape-corner-full, 9999px)));
    border-end-end-radius: var(--lit-material-button-shape-end, var(--lit-material-button-shape, var(--md-sys-shape-corner-full, 9999px)));
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
      background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      transform 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .button:active {
    transform: scale(var(--lit-material-button-pressed-scale, 1));
  }

  @media (prefers-reduced-motion: reduce) {
    .button:active {
      transform: none;
    }
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
`,A=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},$e=0,j=class extends T{static{this.styles=Qe}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`button`,this.disabled=!1,this.name=``,this.value=``,this.href=``,this.target=``,this.ripple=new Xe(this),this.focusRing=new Ze(this),this.labelId=`lit-material-button-label-${++$e}`,this.internals=this.attachInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}render(){let e=x`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      <slot name="icon" class="icon"></slot>
      <span class="label" id=${this.labelId}><slot></slot></span>
    `;return this.href?x`
        <a
          class="button"
          part="button"
          href=${this.disabled?w:this.href}
          target=${this.target||w}
          rel=${this.target===`_blank`?`noopener noreferrer`:w}
          aria-disabled=${this.disabled?`true`:w}
          aria-labelledby=${this.labelId}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${e}
        </a>
      `:x`
      <button
        class="button"
        part="button"
        type="button"
        ?disabled=${this.disabled}
        name=${this.name||w}
        value=${this.value||w}
        aria-labelledby=${this.labelId}
        @click=${this.handleClick}
      >
        ${e}
      </button>
    `}handleClick(e){this.disabled?(e.preventDefault(),e.stopImmediatePropagation()):this.type===`submit`?this.internals.form?.requestSubmit():this.type===`reset`&&this.internals.form?.reset()}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};A([D()],j.prototype,`variant`,void 0),A([D()],j.prototype,`type`,void 0),A([D({type:Boolean,reflect:!0})],j.prototype,`disabled`,void 0),A([D()],j.prototype,`name`,void 0),A([D()],j.prototype,`value`,void 0),A([D({reflect:!0})],j.prototype,`form`,void 0),A([D()],j.prototype,`href`,void 0),A([D()],j.prototype,`target`,void 0),A([k(`.state-layer`)],j.prototype,`stateLayerElement`,void 0),j=A([E(`lit-material-button`)],j);var et=o`
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
`,tt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},nt=class extends T{static{this.styles=et}constructor(){super(),this.variant=`standard`,this.position=`start`,this.selected=-1,this.open=!1,this.disableBackdropClose=!1,this.reopenOnClose=!1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected(),e.has(`open`)&&this.variant===`modal`&&this.dialogElement&&(this.open&&!this.dialogElement.open?this.dialogElement.showModal():!this.open&&this.dialogElement.open&&this.dialogElement.close())}show(){this.open=!0}close(){this.open=!1}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-drawer-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}renderNav(){let e=this.variant===`modal`;return x`
      <nav
        class="drawer ${this.variant}"
        part="drawer"
        tabindex=${e?`-1`:w}
        ?autofocus=${e}
      >
        <div class="header" part="header"><slot name="header"></slot></div>
        <div class="content" part="content"><slot @slotchange=${this.handleSlotChange}></slot></div>
      </nav>
    `}render(){return this.variant===`modal`?x`
      <dialog
        class="dialog"
        part="dialog"
        @close=${this.handleClose}
        @cancel=${this.handleCancel}
        @click=${this.handleBackdropClick}
      >
        ${this.renderNav()}
      </dialog>
    `:this.renderNav()}handleClose(){this.reopenOnClose?(this.reopenOnClose=!1,this.dialogElement?.showModal()):(this.open=!1,this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0})))}handleCancel(e){this.dispatchEvent(new Event(`cancel`,{bubbles:!0,cancelable:!0,composed:!0}))||(e.preventDefault(),this.reopenOnClose=!0)}handleBackdropClick(e){!this.disableBackdropClose&&e.target===this.dialogElement&&this.dialogElement?.close()}};tt([D()],nt.prototype,`variant`,void 0),tt([D({reflect:!0})],nt.prototype,`position`,void 0),tt([D({type:Number})],nt.prototype,`selected`,void 0),tt([D({type:Boolean,reflect:!0})],nt.prototype,`open`,void 0),tt([D({type:Boolean,attribute:`disable-backdrop-close`})],nt.prototype,`disableBackdropClose`,void 0),tt([k(`dialog`)],nt.prototype,`dialogElement`,void 0),nt=tt([E(`lit-material-navigation-drawer`)],nt);var rt=o`
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
`,it=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},at=class extends T{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Xe(this),this.focusRing=new Ze(this)}static{this.styles=rt}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return x`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      <slot name="icon" class="icon"></slot>
      <span class="label"><slot></slot></span>
      <slot name="badge" class="badge"></slot>
    `}render(){let e=this.selected?`page`:w;return this.href?x`
        <a
          class="item"
          part="item"
          href=${this.disabled?w:this.href}
          target=${this.target||w}
          rel=${this.target===`_blank`?`noopener noreferrer`:w}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:w}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:x`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};it([D({type:Boolean,reflect:!0})],at.prototype,`selected`,void 0),it([D({type:Boolean,reflect:!0})],at.prototype,`disabled`,void 0),it([D()],at.prototype,`href`,void 0),it([D()],at.prototype,`target`,void 0),it([k(`.state-layer`)],at.prototype,`stateLayerElement`,void 0),at=it([E(`lit-material-navigation-drawer-item`)],at);var ot=o`
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
`,st=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ct=class extends T{static{this.styles=ot}constructor(){super(),this.alignment=`top`,this.selected=-1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected()}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-rail-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}render(){return x`
      <div class="rail" part="rail">
        <div class="fab" part="fab"><slot name="fab"></slot></div>
        <div class="items ${this.alignment}" part="items">
          <slot @slotchange=${this.handleSlotChange}></slot>
        </div>
      </div>
    `}};st([D()],ct.prototype,`alignment`,void 0),st([D({type:Number})],ct.prototype,`selected`,void 0),ct=st([E(`lit-material-navigation-rail`)],ct);var lt=o`
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
`,ut=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},dt=class extends T{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Xe(this),this.focusRing=new Ze(this)}static{this.styles=lt}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return x`
      <span class="icon-container" part="icon-container">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="state-layer" part="ripple"></div>
        <slot name="icon" class="icon"></slot>
        <slot name="badge" class="badge"></slot>
      </span>
      <span class="label"><slot></slot></span>
    `}render(){let e=this.selected?`page`:w;return this.href?x`
        <a
          class="item"
          part="item"
          href=${this.disabled?w:this.href}
          target=${this.target||w}
          rel=${this.target===`_blank`?`noopener noreferrer`:w}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:w}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:x`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};ut([D({type:Boolean,reflect:!0})],dt.prototype,`selected`,void 0),ut([D({type:Boolean,reflect:!0})],dt.prototype,`disabled`,void 0),ut([D()],dt.prototype,`href`,void 0),ut([D()],dt.prototype,`target`,void 0),ut([k(`.state-layer`)],dt.prototype,`stateLayerElement`,void 0),dt=ut([E(`lit-material-navigation-rail-item`)],dt);var ft=o`
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
`,pt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},mt=class extends T{static{this.styles=ft}constructor(){super(),this.selected=-1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected()}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-bar-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}render(){return x`
      <div class="bar" part="bar">
        <slot @slotchange=${this.handleSlotChange}></slot>
      </div>
    `}};pt([D({type:Number})],mt.prototype,`selected`,void 0),mt=pt([E(`lit-material-navigation-bar`)],mt);var ht=o`
  :host {
    display: block;
    flex: 1;
    /* Shrink below the pill's 64px so more than five destinations fit a phone. */
    min-width: 0;
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
    width: min(64px, 100%);
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
`,gt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},_t=class extends T{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Xe(this),this.focusRing=new Ze(this)}static{this.styles=ht}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return x`
      <span class="icon-container" part="icon-container">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="state-layer" part="ripple"></div>
        <slot name="icon" class="icon"></slot>
        <slot name="badge" class="badge"></slot>
      </span>
      <span class="label"><slot></slot></span>
    `}render(){let e=this.selected?`page`:w;return this.href?x`
        <a
          class="item"
          part="item"
          href=${this.disabled?w:this.href}
          target=${this.target||w}
          rel=${this.target===`_blank`?`noopener noreferrer`:w}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:w}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:x`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};gt([D({type:Boolean,reflect:!0})],_t.prototype,`selected`,void 0),gt([D({type:Boolean,reflect:!0})],_t.prototype,`disabled`,void 0),gt([D()],_t.prototype,`href`,void 0),gt([D()],_t.prototype,`target`,void 0),gt([k(`.state-layer`)],_t.prototype,`stateLayerElement`,void 0),_t=gt([E(`lit-material-navigation-bar-item`)],_t);var vt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},yt=class extends T{constructor(){super(...arguments),this.destinations=[],this.selected=-1,this.expanded=`drawer`,this.label=`Main`}static{this.styles=o`
    :host {
      display: block;
    }
    .bar,
    .rail,
    .drawer {
      display: none;
    }
    @media (max-width: 599.98px) {
      .bar { display: block; }
    }
    @media (min-width: 600px) and (max-width: 839.98px) {
      .rail { display: block; height: 100%; }
    }
    @media (min-width: 840px) {
      :host(:not([expanded="rail"])) .drawer { display: block; height: 100%; }
      :host([expanded="rail"]) .rail { display: block; height: 100%; }
    }
    [slot="icon"] {
      display: inline-flex;
      width: 24px;
      height: 24px;
      align-items: center;
      justify-content: center;
    }
    [slot="icon"] svg {
      width: 100%;
      height: 100%;
    }
  `}onChange(e){e.stopPropagation();let t=e.target.selected;t!==this.selected&&(this.selected=t,this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})))}icon(e){return e.icon===void 0?w:x`<span slot="icon" aria-hidden="true">${e.icon}</span>`}render(){let e=this.destinations;return x`
      <nav aria-label=${this.label}>
        <div class="drawer">
          <lit-material-navigation-drawer .selected=${this.selected} @change=${this.onChange}>
            ${e.map((e,t)=>x`<lit-material-navigation-drawer-item ?selected=${t===this.selected} ?disabled=${!!e.disabled} href=${e.href??w}
              >${this.icon(e)}${e.label}</lit-material-navigation-drawer-item>`)}
          </lit-material-navigation-drawer>
        </div>
        <div class="rail">
          <lit-material-navigation-rail .selected=${this.selected} @change=${this.onChange}>
            ${e.map((e,t)=>x`<lit-material-navigation-rail-item ?selected=${t===this.selected} ?disabled=${!!e.disabled} href=${e.href??w}
              >${this.icon(e)}${e.label}</lit-material-navigation-rail-item>`)}
          </lit-material-navigation-rail>
        </div>
        <div class="bar">
          <lit-material-navigation-bar .selected=${this.selected} @change=${this.onChange}>
            ${e.map((e,t)=>x`<lit-material-navigation-bar-item ?selected=${t===this.selected} ?disabled=${!!e.disabled} href=${e.href??w}
              >${this.icon(e)}${e.label}</lit-material-navigation-bar-item>`)}
          </lit-material-navigation-bar>
        </div>
      </nav>
    `}};vt([D({attribute:!1})],yt.prototype,`destinations`,void 0),vt([D({type:Number})],yt.prototype,`selected`,void 0),vt([D({reflect:!0})],yt.prototype,`expanded`,void 0),vt([D()],yt.prototype,`label`,void 0),yt=vt([E(`lit-material-adaptive-navigation`)],yt);var bt=o`
  :host {
    display: inline-flex;
    vertical-align: middle;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: var(--md-sys-typescale-label-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-label-medium-line-height, 1.25rem);
    font-weight: var(--md-sys-typescale-label-medium-weight, 600);
  }

  .ok { color: var(--lit-material-status-ok-color, light-dark(#3f6b2a, #9fcf86)); }
  .warning { color: var(--lit-material-status-warning-color, light-dark(#7d5300, #f0c060)); }
  .error { color: var(--lit-material-status-error-color, var(--md-sys-color-error, light-dark(#b3261e, #f2b8b5))); }
  .info { color: var(--lit-material-status-info-color, var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff))); }
  .neutral {
    color: var(--lit-material-status-neutral-color, var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0)));
    font-weight: var(--md-sys-typescale-label-medium-weight-prominent, 500);
  }

  .icon {
    display: inline-flex;
    flex: none;
    width: 1.25em;
    height: 1.25em;
  }
  svg {
    width: 100%;
    height: 100%;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  svg .fill { fill: currentColor; stroke: none; }
  ::slotted([slot="icon"]) { width: 100%; height: 100%; }

  @media (forced-colors: active) {
    .status { color: CanvasText; }
  }
`,xt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},St={ok:S`<circle cx="10" cy="10" r="8"/><path d="M6.5 10.2l2.4 2.4 4.6-4.8"/>`,warning:S`<path d="M10 3l7.5 13h-15z"/><path d="M10 8.2v3.6M10 14.2v.1"/>`,error:S`<circle cx="10" cy="10" r="8"/><path d="M7.2 7.2l5.6 5.6M12.8 7.2l-5.6 5.6"/>`,info:S`<circle cx="10" cy="10" r="8"/><path d="M10 9v5M10 6.2v.1"/>`,neutral:S`<circle cx="10" cy="10" r="3.2" class="fill"/>`},Ct=new Set([`ok`,`warning`,`error`,`info`,`neutral`]),wt=class extends T{constructor(){super(...arguments),this.status=`neutral`}static{this.styles=bt}get kind(){return Ct.has(this.status)?this.status:`neutral`}render(){let e=this.kind;return x`<span class="status ${e}" part="status">
      <span class="icon" part="icon" aria-hidden="true"
        ><slot name="icon"><svg viewBox="0 0 20 20" focusable="false">${St[e]}</svg></slot></span
      ><span class="label"><slot></slot></span>
    </span>`}};xt([D({reflect:!0})],wt.prototype,`status`,void 0),wt=xt([E(`lit-material-status`)],wt);var Tt=o`
  :host {
    display: block;
    min-width: 0;
  }

  .stat {
    box-sizing: border-box;
    height: 100%;
    padding: 16px;
    border-radius: var(--md-sys-shape-corner-large, 12px);
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
  }
  .outlined {
    border: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    background: var(--md-sys-color-surface-container-low, light-dark(#f7f2fa, #1d1b20));
  }
  .filled {
    background: var(--md-sys-color-surface-container-highest, light-dark(#e6e0e9, #36343b));
  }
  .plain {
    padding: 0;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
  }
  ::slotted([slot="icon"]) {
    display: inline-flex;
    width: 20px;
    height: 20px;
  }

  .value {
    margin-top: 4px;
    font-family: var(--lit-material-stat-value-font, inherit);
    font-size: var(--lit-material-stat-value-size, 1.5rem);
    line-height: var(--lit-material-stat-value-line-height, 2rem);
    font-weight: var(--lit-material-stat-value-weight, 600);
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }

  .caption {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 8px;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-size: var(--md-sys-typescale-body-small-size, 0.75rem);
    line-height: var(--md-sys-typescale-body-small-line-height, 1rem);
  }
  .caption:empty { display: none; }

  .trend { font-weight: 600; }
  .trend.positive { color: var(--lit-material-stat-positive-color, light-dark(#3f6b2a, #9fcf86)); }
  .trend.negative { color: var(--lit-material-stat-negative-color, var(--md-sys-color-error, light-dark(#b3261e, #f2b8b5))); }
`,Et=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},M=class extends T{constructor(){super(...arguments),this.label=``,this.value=``,this.caption=``,this.trendLabel=``,this.trendSentiment=`neutral`,this.variant=`outlined`}static{this.styles=Tt}arrow(){switch(this.trend){case`up`:return`↑`;case`down`:return`↓`;case`flat`:return`→`;default:return``}}render(){let e=this.trend&&this.trendLabel;return x`
      <div class="stat ${this.variant}" part="stat" role="group" aria-label=${this.label||w}>
        <div class="label" part="label"><slot name="icon"></slot><span>${this.label}</span></div>
        <div class="value" part="value">${this.value}</div>
        <div class="caption" part="caption">
          ${e?x`<span class="trend ${this.trendSentiment}"><span aria-hidden="true">${this.arrow()}</span> ${this.trendLabel}</span>`:w}
          <slot name="caption">${this.caption?x`<span>${this.caption}</span>`:w}</slot>
        </div>
      </div>
    `}};Et([D()],M.prototype,`label`,void 0),Et([D()],M.prototype,`value`,void 0),Et([D()],M.prototype,`caption`,void 0),Et([D()],M.prototype,`trend`,void 0),Et([D({attribute:`trend-label`})],M.prototype,`trendLabel`,void 0),Et([D({attribute:`trend-sentiment`})],M.prototype,`trendSentiment`,void 0),Et([D({reflect:!0})],M.prototype,`variant`,void 0),M=Et([E(`lit-material-stat`)],M);var Dt=o`
  :host {
    display: block;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    gap: 8px;
    box-sizing: border-box;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
  }
  .standard {
    align-items: center;
    text-align: center;
    padding: 48px 24px;
    max-width: 480px;
    margin-inline: auto;
  }
  .compact {
    align-items: flex-start;
    padding: 24px;
    border: 1px dashed var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    border-radius: var(--md-sys-shape-corner-large, 12px);
  }

  .headline {
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-title-medium-font, inherit);
    font-size: var(--md-sys-typescale-title-medium-size, 1rem);
    line-height: var(--md-sys-typescale-title-medium-line-height, 1.5rem);
    font-weight: var(--md-sys-typescale-title-medium-weight, 600);
    text-wrap: balance;
  }
  .body { text-wrap: pretty; }

  .icon, .actions { display: flex; }
  .icon { color: var(--md-sys-color-outline, light-dark(#79747e, #938f99)); margin-bottom: 8px; }
  .actions { flex-wrap: wrap; gap: 8px; margin-top: 8px; }
  .standard .actions { justify-content: center; }
  .icon.empty, .actions.empty { display: none; }
  ::slotted([slot="icon"]) { width: 48px; height: 48px; }
  .compact ::slotted([slot="icon"]) { width: 32px; height: 32px; }
`,Ot=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},kt=class extends T{constructor(){super(...arguments),this.headline=``,this.level=2,this.size=`standard`,this.emptyIcon=!1,this.emptyActions=!1}static{this.styles=Dt}onSlotChange(e){let t=e.target,n=t.assignedNodes({flatten:!0}).length===0;t.name===`icon`?this.emptyIcon=n:this.emptyActions=n}firstUpdated(){for(let e of this.renderRoot.querySelectorAll(`slot[name]`))e.dispatchEvent(new Event(`slotchange`))}render(){let e=Math.min(6,Math.max(1,Math.round(this.level||2)));return x`
      <div class="empty-state ${this.size}" part="empty-state">
        <div class="icon ${this.emptyIcon?`empty`:``}"><slot name="icon" @slotchange=${this.onSlotChange}></slot></div>
        ${this.headline?x`<div class="headline" part="headline" role="heading" aria-level=${e}>${this.headline}</div>`:w}
        <div class="body"><slot></slot></div>
        <div class="actions ${this.emptyActions?`empty`:``}"><slot name="actions" @slotchange=${this.onSlotChange}></slot></div>
      </div>
    `}};Ot([D()],kt.prototype,`headline`,void 0),Ot([D({type:Number})],kt.prototype,`level`,void 0),Ot([D({reflect:!0})],kt.prototype,`size`,void 0),Ot([O()],kt.prototype,`emptyIcon`,void 0),Ot([O()],kt.prototype,`emptyActions`,void 0),kt=Ot([E(`lit-material-empty-state`)],kt);var At={ATTRIBUTE:1,CHILD:2,PROPERTY:3,BOOLEAN_ATTRIBUTE:4,EVENT:5,ELEMENT:6},jt=e=>(...t)=>({_$litDirective$:e,values:t}),Mt=class{constructor(e){}get _$AU(){return this._$AM._$AU}_$AT(e,t,n){this._$Ct=e,this._$AM=t,this._$Ci=n}_$AS(e,t){return this.update(e,t)}update(e,t){return this.render(...t)}},{I:Nt}=Re,Pt=e=>e,Ft=()=>document.createComment(``),It=(e,t,n)=>{let r=e._$AA.parentNode,i=t===void 0?e._$AB:t._$AA;if(n===void 0)n=new Nt(r.insertBefore(Ft(),i),r.insertBefore(Ft(),i),e,e.options);else{let t=n._$AB.nextSibling,a=n._$AM,o=a!==e;if(o){let t;n._$AQ?.(e),n._$AM=e,n._$AP!==void 0&&(t=e._$AU)!==a._$AU&&n._$AP(t)}if(t!==i||o){let e=n._$AA;for(;e!==t;){let t=Pt(e).nextSibling;Pt(r).insertBefore(e,i),e=t}}}return n},Lt=(e,t,n=e)=>(e._$AI(t,n),e),Rt={},zt=(e,t=Rt)=>e._$AH=t,Bt=e=>e._$AH,Vt=e=>{e._$AR(),e._$AA.remove()},Ht=(e,t,n)=>{let r=new Map;for(let i=t;i<=n;i++)r.set(e[i],i);return r},Ut=jt(class extends Mt{constructor(e){if(super(e),e.type!==At.CHILD)throw Error(`repeat() can only be used in text expressions`)}dt(e,t,n){let r;n===void 0?n=t:t!==void 0&&(r=t);let i=[],a=[],o=0;for(let t of e)i[o]=r?r(t,o):o,a[o]=n(t,o),o++;return{values:a,keys:i}}render(e,t,n){return this.dt(e,t,n).values}update(e,[t,n,r]){let i=Bt(e),{values:a,keys:o}=this.dt(t,n,r);if(!Array.isArray(i))return this.ut=o,a;let s=this.ut??=[],c=[],l,u,d=0,f=i.length-1,p=0,m=a.length-1;for(;d<=f&&p<=m;)if(i[d]===null)d++;else if(i[f]===null)f--;else if(s[d]===o[p])c[p]=Lt(i[d],a[p]),d++,p++;else if(s[f]===o[m])c[m]=Lt(i[f],a[m]),f--,m--;else if(s[d]===o[m])c[m]=Lt(i[d],a[m]),It(e,c[m+1],i[d]),d++,m--;else if(s[f]===o[p])c[p]=Lt(i[f],a[p]),It(e,i[d],i[f]),f--,p++;else if(l===void 0&&(l=Ht(o,p,m),u=Ht(s,d,f)),l.has(s[d])){if(l.has(s[f])){let t=u.get(o[p]),n=t===void 0?null:i[t];if(n===null){let t=It(e,i[d]);Lt(t,a[p]),c[p]=t}else c[p]=Lt(n,a[p]),It(e,i[d],n),i[t]=null;p++}else Vt(i[f]),f--}else Vt(i[d]),d++;for(;p<=m;){let t=It(e,c[m+1]);Lt(t,a[p]),c[p++]=t}for(;d<=f;){let e=i[d++];e!==null&&Vt(e)}return this.ut=o,zt(e,c),C}}),Wt=o`
  :host {
    display: block;
  }
  .box {
    position: relative;
  }
  .viewport {
    box-sizing: border-box;
    height: var(--lit-material-log-viewer-height, 320px);
    overflow: auto;
    padding: 8px 0;
    border: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    border-radius: var(--md-sys-shape-corner-large, 12px);
    background: var(--md-sys-color-surface-container-lowest, light-dark(#ffffff, #0f0d13));
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--lit-material-log-viewer-font, ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace);
    font-size: var(--lit-material-log-viewer-font-size, 0.75rem);
    line-height: 1.6;
  }
  .viewport:focus-visible {
    outline: 2px solid var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
    outline-offset: 2px;
  }
  .line {
    position: relative;
    padding: 0 12px 0 20px;
    white-space: pre;
  }
  :host([wrap]) .line {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .line:hover {
    background: var(--md-sys-color-surface-container, light-dark(#f3edf7, #211f26));
  }
  .mark {
    position: absolute;
    left: 8px;
    top: 0.5em;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .error { color: var(--lit-material-log-viewer-error-color, var(--md-sys-color-error, light-dark(#b3261e, #f2b8b5))); }
  .warning { color: var(--lit-material-log-viewer-warning-color, light-dark(#7d5300, #f0c060)); }
  .warning .mark { border-radius: 0; transform: rotate(45deg); }
  .info { color: var(--lit-material-log-viewer-info-color, var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff))); }
  .info .mark { border-radius: 1px; }

  .jump {
    position: absolute;
    left: 50%;
    bottom: 12px;
    transform: translateX(-50%);
    border: 0;
    border-radius: var(--md-sys-shape-corner-full, 9999px);
    padding: 6px 12px;
    background: var(--md-sys-color-inverse-surface, light-dark(#322f35, #e6e0e9));
    color: var(--md-sys-color-inverse-on-surface, light-dark(#f5eff7, #322f35));
    font: inherit;
    font-family: var(--md-sys-typescale-label-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-label-medium-size, 0.75rem);
    font-weight: 600;
    cursor: pointer;
  }
  .jump:focus-visible {
    outline: 2px solid var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
    outline-offset: 2px;
  }
  .held {
    margin-top: 8px;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-family: var(--md-sys-typescale-body-small-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-small-size, 0.75rem);
  }
`,N=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},P=class extends T{constructor(){super(...arguments),this.maxLines=1e3,this.paused=!1,this.wrap=!1,this.label=`Log`,this.entries=[],this.unseen=0,this.held=[],this.nextId=0,this.following=!0}static{this.styles=Wt}get lines(){return this.entries.map(e=>e.text)}set lines(e){this.entries=[],this.held=[],this.unseen=0,this.following=!0,this.add(e)}appendLines(e){let t=(Array.isArray(e)?e:[e]).flatMap(e=>String(e).split(`
`));this.paused?this.held=[...this.held,...t].slice(-this.maxLines):this.add(t)}clear(){this.entries=[],this.held=[],this.unseen=0}scrollToLatest(){this.following=!0,this.unseen=0;let e=this.viewport;e&&(e.scrollTop=e.scrollHeight)}add(e){if(e.length===0)return;let t=e.map(e=>({id:this.nextId++,text:e,level:this.highlight?.(e)??void 0})),n=[...this.entries,...t],r=Math.max(0,n.length-this.maxLines);this.entries=r?n.slice(r):n,r&&this.dispatchEvent(new CustomEvent(`lines-dropped`,{detail:{count:r},bubbles:!0,composed:!0})),this.following||(this.unseen+=t.length)}willUpdate(e){if(e.has(`paused`)&&!this.paused&&this.held.length){let e=this.held;this.held=[],this.add(e)}}updated(){this.following&&this.viewport&&(this.viewport.scrollTop=this.viewport.scrollHeight)}onScroll(){let e=this.viewport;e&&(this.following=e.scrollHeight-e.scrollTop-e.clientHeight<24,this.following&&(this.unseen=0))}render(){return x`
      <div class="box">
        <div class="viewport" part="viewport" role="log" aria-live="off" aria-label=${this.label} tabindex="0" @scroll=${this.onScroll}>
          ${Ut(this.entries,e=>e.id,e=>x`<div class="line ${e.level??``}" part="line">${e.level?x`<span class="mark" aria-label=${e.level}></span>`:w}${e.text}</div>`)}
        </div>
        ${this.unseen>0?x`<button class="jump" part="jump" @click=${()=>this.scrollToLatest()}>${this.unseen} new line${this.unseen===1?``:`s`} ↓</button>`:w}
        ${this.paused&&this.held.length?x`<div class="held" role="status">Paused: ${this.held.length} line${this.held.length===1?``:`s`} waiting</div>`:w}
      </div>
    `}};N([D({type:Number,attribute:`max-lines`})],P.prototype,`maxLines`,void 0),N([D({type:Boolean,reflect:!0})],P.prototype,`paused`,void 0),N([D({type:Boolean,reflect:!0})],P.prototype,`wrap`,void 0),N([D()],P.prototype,`label`,void 0),N([D({attribute:!1})],P.prototype,`highlight`,void 0),N([O()],P.prototype,`entries`,void 0),N([O()],P.prototype,`unseen`,void 0),N([O()],P.prototype,`held`,void 0),N([k(`.viewport`)],P.prototype,`viewport`,void 0),P=N([E(`lit-material-log-viewer`)],P);var Gt=o`
  :host {
    display: block;
    min-width: 0;
  }
  .container {
    overflow: auto;
    max-height: inherit;
    height: 100%;
    border: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    border-radius: var(--md-sys-shape-corner-large, 12px);
    background: var(--md-sys-color-surface, light-dark(#fef7ff, #141218));
  }
  .container:focus-visible {
    outline: 2px solid var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
    outline-offset: 2px;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
  }
  caption {
    caption-side: top;
    text-align: left;
    padding: 12px 16px 0;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-size: var(--md-sys-typescale-body-small-size, 0.75rem);
  }
  th,
  td {
    text-align: left;
    padding: 12px 16px;
    border-bottom: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    vertical-align: middle;
  }
  :host([density="compact"]) th,
  :host([density="compact"]) td {
    padding: 6px 12px;
  }
  tbody tr:last-child td {
    border-bottom: 0;
  }
  th {
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    background: var(--md-sys-color-surface-container-low, light-dark(#f7f2fa, #1d1b20));
    font-weight: 600;
    white-space: nowrap;
  }
  :host([sticky-header]) th {
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .mono {
    font-family: var(--lit-material-table-mono-font, ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace);
    overflow-wrap: anywhere;
  }
  .empty {
    padding: 24px 16px;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
  }
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
`,Kt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},qt=class extends T{constructor(){super(...arguments),this.columns=[],this.rows=[],this.caption=``,this.captionHidden=!1,this.density=`standard`,this.stickyHeader=!1}static{this.styles=Gt}cell(e,t){if(t.render)return t.render(e);let n=e[t.key];return n==null?``:String(n)}render(){let e=this.columns;return x`
      <div class="container" part="container" tabindex=${this.stickyHeader?`0`:w}>
        <table part="table">
          ${this.caption?x`<caption class=${this.captionHidden?`visually-hidden`:``}>${this.caption}</caption>`:w}
          <thead>
            <tr>
              ${e.map(e=>x`<th scope="col" class=${[e.numeric?`num`:``,e.hiddenLabel?`hidden-label`:``].join(` `).trim()||w}>
                    ${e.hiddenLabel?x`<span class="visually-hidden">${e.label}</span>`:e.label}
                  </th>`)}
            </tr>
          </thead>
          ${this.rows.length?x`<tbody>
                ${this.rows.map(t=>x`<tr>
                      ${e.map(e=>x`<td class=${[e.numeric?`num`:``,e.mono?`mono`:``].join(` `).trim()||w}>${this.cell(t,e)}</td>`)}
                    </tr>`)}
              </tbody>`:w}
        </table>
        ${this.rows.length?w:x`<div class="empty"><slot name="empty"></slot></div>`}
      </div>
    `}};Kt([D({attribute:!1})],qt.prototype,`columns`,void 0),Kt([D({attribute:!1})],qt.prototype,`rows`,void 0),Kt([D()],qt.prototype,`caption`,void 0),Kt([D({type:Boolean,attribute:`caption-hidden`})],qt.prototype,`captionHidden`,void 0),Kt([D({reflect:!0})],qt.prototype,`density`,void 0),Kt([D({type:Boolean,reflect:!0,attribute:`sticky-header`})],qt.prototype,`stickyHeader`,void 0),qt=Kt([E(`lit-material-table`)],qt);function Jt(e,t){if(!(e>0)||!Number.isFinite(e))return 1;let n=e/Math.max(1,t),r=10**Math.floor(Math.log10(n));for(let e of[1,2,2.5,5,10])if(e*r>=n)return e*r;return 10*r}function Yt(e,t,n=4){if(!Number.isFinite(e)||!Number.isFinite(t))return{ticks:[0,1],min:0,max:1};if(e===t){let n=e===0?1:Math.abs(e)*.1;e-=n,t+=n}let r=Jt(t-e,n),i=Math.floor(e/r)*r,a=Math.ceil(t/r)*r,o=[];for(let e=i,t=0;e<=a+r/2&&t<n*3+2;e+=r,t++)o.push(Number(e.toPrecision(12)));return{ticks:o,min:i,max:a}}function Xt(e,t,n=5){return!Number.isFinite(e)||!Number.isFinite(t)?[]:e===t?[e]:Array.from({length:n},(r,i)=>e+(t-e)*i/(n-1))}function Zt(e){if(!Number.isFinite(e))return``;let t=Math.abs(e);return t>=1e9?`${Qt(e/1e9)}G`:t>=1e6?`${Qt(e/1e6)}M`:t>=1e4?`${Qt(e/1e3)}k`:t>=100||Number.isInteger(e)?String(Math.round(e)):Qt(e)}function Qt(e){return String(Number(e.toPrecision(3)))}function $t(e,t,n,r){let i=e.map((e,t)=>({p:e,i:t})).sort((e,t)=>e.p-t.p),a=[...e],o=-1/0;for(let{p:e,i:r}of i){let i=Math.max(e,o+t,n);a[r]=i,o=i}let s=Math.max(0,Math.max(...a)-r);return s?a.map(e=>e-s):a}var en=a([[`#2a78d6`,`#3987e5`],[`#eb6834`,`#d95926`],[`#1baf7a`,`#199e70`],[`#eda100`,`#c98500`],[`#e87ba4`,`#d55181`],[`#008300`,`#008300`],[`#6250d6`,`#9085e9`],[`#e34948`,`#e66767`]].map(([e,t],n)=>`.s${n+1} { --series: var(--lit-material-chart-series-${n+1}, light-dark(${e}, ${t})); }`).join(`
`)),tn=o`
  :host {
    --chart-text: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    --chart-muted: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    --chart-grid: var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    --chart-surface: var(--md-sys-color-surface, light-dark(#fef7ff, #141218));
  }
`,nn=[tn,en,o`
    :host {
      display: block;
      min-width: 0;
      color: var(--chart-text);
      font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    }
    .chart {
      position: relative;
    }
    svg {
      display: block;
      width: 100%;
      height: auto;
      overflow: visible;
      touch-action: pan-y;
    }
    svg:focus-visible {
      outline: 2px solid var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
      outline-offset: 4px;
      border-radius: 4px;
    }
    .grid {
      stroke: var(--chart-grid);
      stroke-width: 1;
    }
    .axis,
    .end-label {
      fill: var(--chart-muted);
      font-size: 12px;
      font-variant-numeric: tabular-nums;
    }
    .end-label {
      fill: var(--chart-text);
    }
    .line {
      fill: none;
      stroke: var(--series);
      stroke-width: 2;
      stroke-linejoin: round;
      stroke-linecap: round;
    }
    .dot,
    .marker {
      fill: var(--series);
      stroke: var(--chart-surface);
      stroke-width: 2;
    }
    .crosshair {
      stroke: var(--chart-muted);
      stroke-width: 1;
    }

    .legend {
      display: flex;
      flex-wrap: wrap;
      gap: 4px 16px;
      margin: 0 0 8px;
      padding: 0;
      list-style: none;
      color: var(--chart-text);
      font-size: var(--md-sys-typescale-body-small-size, 0.75rem);
      line-height: var(--md-sys-typescale-body-small-line-height, 1rem);
    }
    .legend li {
      display: inline-flex;
      align-items: center;
      gap: 6px;
    }
    .swatch {
      display: inline-block;
      width: 12px;
      height: 3px;
      border-radius: 2px;
      background: var(--series);
    }

    .tooltip {
      position: absolute;
      top: 0;
      transform: translateX(-50%);
      pointer-events: none;
      padding: 6px 8px;
      border-radius: var(--md-sys-shape-corner-small, 8px);
      background: var(--md-sys-color-inverse-surface, light-dark(#322f35, #e6e0e9));
      color: var(--md-sys-color-inverse-on-surface, light-dark(#f5eff7, #322f35));
      font-size: var(--md-sys-typescale-body-small-size, 0.75rem);
      line-height: var(--md-sys-typescale-body-small-line-height, 1rem);
      font-variant-numeric: tabular-nums;
      white-space: nowrap;
    }
    .tip-x {
      opacity: 0.8;
    }
    .tip-row {
      display: flex;
      align-items: center;
      gap: 6px;
    }

    .empty {
      display: grid;
      place-items: center;
      min-height: 120px;
      border: 1px dashed var(--chart-grid);
      border-radius: var(--md-sys-shape-corner-large, 12px);
      color: var(--chart-muted);
      font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    }

    .visually-hidden {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    @media (forced-colors: active) {
      .line { stroke: CanvasText; }
      .swatch, .dot, .marker { background: CanvasText; fill: CanvasText; }
    }
  `],F=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},rn=4,an=8,on=28,sn=40,I=class extends T{constructor(){super(...arguments),this.series=[],this.label=``,this.height=240,this.formatX=Zt,this.formatY=Zt,this.unit=``,this.width=640,this.active=-1}static{this.styles=nn}connectedCallback(){super.connectedCallback(),typeof ResizeObserver<`u`&&(this.observer=new ResizeObserver(e=>{let t=Math.round(e[0]?.contentRect.width??0);t>0&&t!==this.width&&requestAnimationFrame(()=>this.width=t)}),this.observer.observe(this))}disconnectedCallback(){super.disconnectedCallback(),this.observer?.disconnect()}get drawn(){return this.series.length>8&&console.warn(`lit-material-line-chart: ${this.series.length} series; only the first 8 are drawn`),this.series.slice(0,8).map(e=>({name:e.name,points:e.points.filter(e=>Number.isFinite(e.x)&&Number.isFinite(e.y)).sort((e,t)=>e.x-t.x)}))}xsOf(e){let t=new Set;for(let n of e)for(let e of n.points)t.add(e.x);return[...t].sort((e,t)=>e-t)}nearest(e,t){let n;for(let r of e)(!n||Math.abs(r.x-t)<Math.abs(n.x-t))&&(n=r);return n}onPointer(e,t,n){let r=e.currentTarget.getBoundingClientRect(),i=(e.clientX-r.left)/Math.max(1,r.width)*this.width,a=0;for(let e=1;e<t.length;e++)Math.abs(n(t[e])-i)<Math.abs(n(t[a])-i)&&(a=e);this.active=a}onKey(e,t){let n={ArrowLeft:-1,ArrowRight:1,Home:-1/0,End:1/0};if(e.key===`Escape`){this.active=-1;return}if(!(e.key in n))return;e.preventDefault();let r=n[e.key],i=this.active<0?t-1:this.active;this.active=Math.min(t-1,Math.max(0,Number.isFinite(r)?i+r:r<0?0:t-1))}summary(e){return e.map(e=>{let t=e.points.map(e=>e.y);if(!t.length)return`${e.name}: no data`;let n=t[t.length-1];return`${e.name}: latest ${this.formatY(n)}${this.unit?` ${this.unit}`:``}, range ${this.formatY(Math.min(...t))} to ${this.formatY(Math.max(...t))}`}).join(`; `)}render(){let e=this.drawn,t=this.xsOf(e),n=this.label||`Line chart`;if(t.length===0)return x`<div class="empty" role="img" aria-label="${n}: no data">No data</div>`;let r=e.flatMap(e=>e.points.map(e=>e.y)),i=Math.min(...r),a=this.yMin??Math.min(0,i),{ticks:o,min:s,max:c}=Yt(Math.min(a,i),Math.max(...r),4),l=e.length>1&&e.length<=rn,u=Math.max(0,...e.map(e=>e.name.length)),d=l?Math.min(160,12+u*7):12,f=o.map(e=>this.formatY(e)),p=Math.max(sn,12+Math.max(...f.map(e=>e.length))*7),m=this.width,h=Math.max(80,this.height),ee=Math.max(1,m-p-d),te=Math.max(1,h-an-on),ne=t[0],g=t[t.length-1],_=e=>p+(g===ne?ee/2:(e-ne)/(g-ne)*ee),v=e=>an+(1-(e-s)/(c-s||1))*te,re=Xt(ne,g,Math.max(2,Math.min(6,Math.floor(ee/110)+1))),ie=e.map(e=>e.points.length?v(e.points[e.points.length-1].y):an),ae=l?$t(ie,14,14,an+te):[],y=this.active>=0&&this.active<t.length?t[this.active]:void 0,oe=y===void 0?[]:e.map((e,t)=>({i:t,name:e.name,point:this.nearest(e.points,y)})).filter(e=>e.point),se=y===void 0?0:_(y)/m*100;return x`
      ${e.length>1?x`<ul class="legend" part="legend" aria-hidden="true">
            ${e.map((e,t)=>x`<li><span class="swatch s${t+1}"></span>${e.name}</li>`)}
          </ul>`:w}
      <div class="chart" part="chart">
        <svg
          viewBox="0 0 ${m} ${h}"
          width=${m}
          height=${h}
          role="img"
          aria-label="${n}. ${this.summary(e)}"
          tabindex="0"
          @pointermove=${e=>this.onPointer(e,t,_)}
          @pointerdown=${e=>this.onPointer(e,t,_)}
          @pointerleave=${()=>this.active=-1}
          @keydown=${e=>this.onKey(e,t.length)}
          @blur=${()=>this.active=-1}
        >
          ${o.map((e,t)=>S`
              <line class="grid" x1=${p} x2=${p+ee} y1=${v(e)} y2=${v(e)}></line>
              <text class="axis" x=${p-8} y=${v(e)+4} text-anchor="end">${f[t]}</text>`)}
          ${re.map((e,t)=>S`<text class="axis" x=${_(e)} y=${h-8}
              text-anchor=${t===0?`start`:t===re.length-1?`end`:`middle`}>${this.formatX(e)}</text>`)}
          ${e.map((e,t)=>e.points.length===1?S`<circle class="dot s${t+1}" r="4" cx=${_(e.points[0].x)} cy=${v(e.points[0].y)}></circle>`:S`<polyline class="line s${t+1}" points=${e.points.map(e=>`${_(e.x).toFixed(1)},${v(e.y).toFixed(1)}`).join(` `)}></polyline>`)}
          ${l?e.map((e,t)=>S`<text class="end-label" x=${p+ee+6} y=${(ae[t]??an)+4}>${e.name}</text>`):w}
          ${y===void 0?w:S`<line class="crosshair" x1=${_(y)} x2=${_(y)} y1=${an} y2=${an+te}></line>
                ${oe.map(e=>S`<circle class="marker s${e.i+1}" r="4" cx=${_(e.point.x)} cy=${v(e.point.y)}></circle>`)}`}
        </svg>
        ${y===void 0?w:x`<div class="tooltip" part="tooltip" role="status" style="left: ${se}%">
              <div class="tip-x">${this.formatX(y)}</div>
              ${oe.map(t=>x`<div class="tip-row">${e.length>1?x`<span class="swatch s${t.i+1}"></span>${t.name}: `:w}<strong
                    >${this.formatY(t.point.y)}</strong
                  >${this.unit?` ${this.unit}`:``}</div>`)}
            </div>`}
      </div>
      <table class="visually-hidden">
        <caption>${n}</caption>
        <thead>
          <tr>
            <th scope="col">x</th>
            ${e.map(e=>x`<th scope="col">${e.name}${this.unit?` (${this.unit})`:``}</th>`)}
          </tr>
        </thead>
        <tbody>
          ${t.slice(-200).map(t=>x`<tr>
                <th scope="row">${this.formatX(t)}</th>
                ${e.map(e=>{let n=e.points.find(e=>e.x===t);return x`<td>${n?this.formatY(n.y):``}</td>`})}
              </tr>`)}
        </tbody>
      </table>
    `}};F([D({attribute:!1})],I.prototype,`series`,void 0),F([D()],I.prototype,`label`,void 0),F([D({type:Number})],I.prototype,`height`,void 0),F([D({type:Number,attribute:`y-min`})],I.prototype,`yMin`,void 0),F([D({attribute:!1})],I.prototype,`formatX`,void 0),F([D({attribute:!1})],I.prototype,`formatY`,void 0),F([D()],I.prototype,`unit`,void 0),F([O()],I.prototype,`width`,void 0),F([O()],I.prototype,`active`,void 0),I=F([E(`lit-material-line-chart`)],I);var cn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ln=class extends T{constructor(){super(...arguments),this.values=[],this.label=``,this.showLast=!1,this.format=Zt}static{this.styles=[tn,en,o`
      :host {
        display: inline-block;
        width: var(--lit-material-sparkline-width, 96px);
        height: var(--lit-material-sparkline-height, 24px);
        vertical-align: middle;
      }
      svg {
        display: block;
        width: 100%;
        height: 100%;
        overflow: visible;
      }
      polyline {
        fill: none;
        stroke: var(--series);
        stroke-width: 2;
        stroke-linejoin: round;
        stroke-linecap: round;
        vector-effect: non-scaling-stroke;
      }
      circle {
        fill: var(--series);
      }
      @media (forced-colors: active) {
        polyline { stroke: CanvasText; }
        circle { fill: CanvasText; }
      }
    `]}render(){let e=this.values.filter(e=>Number.isFinite(e)),t=this.label||`Trend`;if(e.length===0)return x`<svg role="img" aria-label="${t}: no data"></svg>`;let n=Math.min(...e),r=Math.max(...e),i=r-n||1,a=t=>e.length===1?50:t/(e.length-1)*100,o=e=>2+(1-(e-n)/i)*20,s=e[e.length-1];return x`<svg class="s1" viewBox="0 0 ${100} ${24}" preserveAspectRatio="none" role="img"
      aria-label="${t}: latest ${this.format(s)}, from ${this.format(n)} to ${this.format(r)}">
      <polyline points=${e.map((e,t)=>`${a(t).toFixed(2)},${o(e).toFixed(2)}`).join(` `)}></polyline>
      ${this.showLast||e.length===1?S`<circle cx=${a(e.length-1)} cy=${o(s)} r="2.5"></circle>`:w}
    </svg>`}};cn([D({attribute:!1})],ln.prototype,`values`,void 0),cn([D()],ln.prototype,`label`,void 0),cn([D({type:Boolean,attribute:`show-last`})],ln.prototype,`showLast`,void 0),cn([D({attribute:!1})],ln.prototype,`format`,void 0),ln=cn([E(`lit-material-sparkline`)],ln);var un=[tn,o`
    /* Bars are sized in percentages, so there's no intrinsic width: fill the
       line in flex/grid parents too. */
    :host {
      display: block;
      width: 100%;
      box-sizing: border-box;
      min-width: 0;
      color: var(--chart-text);
      font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
      font-size: var(--md-sys-typescale-body-small-size, 0.75rem);
      line-height: var(--md-sys-typescale-body-small-line-height, 1rem);
      --accent: var(--lit-material-chart-series-1, light-dark(#2a78d6, #3987e5));
    }
    .chart {
      position: relative;
      display: grid;
      gap: 6px;
      /* One row (20px) plus the gap: the tooltip's vertical step. */
      --row-pitch: 26px;
      border-radius: 4px;
    }
    .chart:focus-visible {
      outline: 2px solid var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
      outline-offset: 4px;
    }
    .row {
      display: grid;
      grid-template-columns: var(--lit-material-bar-chart-name-width, 112px) minmax(0, 1fr);
      align-items: center;
      gap: 8px;
      height: 20px;
    }
    .name {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      color: var(--chart-muted);
    }
    .highlight .name {
      color: var(--chart-text);
      font-weight: 600;
    }
    /* Room for a value label after the longest bar, so lengths stay
       comparable across rows. */
    .track {
      position: relative;
      display: block;
      height: 100%;
      --label: 88px;
    }
    .bar {
      position: absolute;
      left: 0;
      top: 2px;
      height: 16px;
      width: max(2px, calc((100% - var(--label)) * var(--f)));
      border-radius: 0 4px 4px 0;
      background: var(--accent);
      transition: opacity 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
    }
    .highlight .bar {
      background: var(--lit-material-bar-chart-highlight, var(--accent));
    }
    .muted .bar {
      background: var(--lit-material-bar-chart-muted, var(--chart-grid));
    }
    .chart:has(.active) .row:not(.active) .bar {
      opacity: 0.55;
    }
    .value {
      position: absolute;
      top: 2px;
      left: calc((100% - var(--label)) * var(--f) + 6px);
      white-space: nowrap;
      font-variant-numeric: tabular-nums;
      font-weight: 600;
    }

    .tooltip {
      position: absolute;
      right: 0;
      transform: translateY(-100%);
      pointer-events: none;
      padding: 6px 8px;
      border-radius: var(--md-sys-shape-corner-small, 8px);
      background: var(--md-sys-color-inverse-surface, light-dark(#322f35, #e6e0e9));
      color: var(--md-sys-color-inverse-on-surface, light-dark(#f5eff7, #322f35));
      font-variant-numeric: tabular-nums;
      white-space: nowrap;
    }

    .empty {
      display: grid;
      place-items: center;
      min-height: 120px;
      border: 1px dashed var(--chart-grid);
      border-radius: var(--md-sys-shape-corner-large, 12px);
      color: var(--chart-muted);
      font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    }

    .visually-hidden {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    @media (prefers-reduced-motion: reduce) {
      .bar { transition: none; }
    }
    @media (forced-colors: active) {
      .bar { background: CanvasText; }
      .highlight .bar { background: Highlight; }
    }
  `],dn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},L=class extends T{constructor(){super(...arguments),this.data=[],this.label=``,this.highlight=``,this.sort=``,this.unit=``,this.format=Zt,this.active=-1}static{this.styles=un}get rows(){let e=this.data.filter(e=>Number.isFinite(e.value));return this.sort===`desc`?e.sort((e,t)=>t.value-e.value):this.sort===`asc`&&e.sort((e,t)=>e.value-t.value),e}text(e){return`${this.format(e.value)}${this.unit?` ${this.unit}`:``}`}onKey(e,t){if(e.key===`Escape`){this.active=-1;return}let n={ArrowDown:e=>e+1,ArrowUp:e=>e<0?t-1:e-1,Home:()=>0,End:()=>t-1}[e.key];n&&(e.preventDefault(),this.active=Math.min(t-1,Math.max(0,n(this.active))))}render(){let e=this.rows,t=this.label||`Bar chart`;if(e.length===0)return x`<div class="empty" role="img" aria-label="${t}: no data">No data</div>`;let n=Math.max(0,...e.map(e=>e.value))||1,r=this.highlight!==``&&e.some(e=>e.name===this.highlight),i=e[this.active];return x`
      <div
        class="chart"
        part="chart"
        role="img"
        aria-label="${t}. ${e.map(e=>`${e.name}: ${this.text(e)}`).join(`; `)}"
        tabindex="0"
        @keydown=${t=>this.onKey(t,e.length)}
        @pointerleave=${()=>this.active=-1}
        @blur=${()=>this.active=-1}
      >
        ${e.map((e,t)=>{let i=r&&e.name===this.highlight,a=Math.max(0,e.value)/n;return x`<div class=${[`row`,i?`highlight`:``,r&&!i?`muted`:``,t===this.active?`active`:``].join(` `)} aria-hidden="true" @pointerenter=${()=>this.active=t} @pointerdown=${()=>this.active=t}>
            <span class="name">${e.name}</span>
            <span class="track" style="--f: ${a.toFixed(4)}">
              <span class="bar"></span>
              ${!r||i?x`<span class="value">${this.text(e)}</span>`:w}
            </span>
          </div>`})}
        ${i?x`<div class="tooltip" part="tooltip" role="status" style="top: calc(${this.active} * var(--row-pitch))">
              ${i.name}: <strong>${this.text(i)}</strong>
            </div>`:w}
      </div>
      <table class="visually-hidden">
        <caption>${t}</caption>
        <thead>
          <tr>
            <th scope="col">Name</th>
            <th scope="col">Value${this.unit?` (${this.unit})`:``}</th>
          </tr>
        </thead>
        <tbody>
          ${e.map(e=>x`<tr><th scope="row">${e.name}</th><td>${this.format(e.value)}</td></tr>`)}
        </tbody>
      </table>
    `}};dn([D({attribute:!1})],L.prototype,`data`,void 0),dn([D()],L.prototype,`label`,void 0),dn([D()],L.prototype,`highlight`,void 0),dn([D()],L.prototype,`sort`,void 0),dn([D()],L.prototype,`unit`,void 0),dn([D({attribute:!1})],L.prototype,`format`,void 0),dn([O()],L.prototype,`active`,void 0),L=dn([E(`lit-material-bar-chart`)],L);var fn=o`
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
`,pn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},mn=class extends T{constructor(){super(...arguments),this.variant=`text`,this.animation=`pulse`,this.width=``,this.height=``}static{this.styles=fn}render(){return x`<span class="skeleton" part="skeleton" style=${`${this.width?`width: ${this.width};`:``}${this.height?`height: ${this.height};`:``}`||w} aria-hidden="true"></span>`}};pn([D({reflect:!0})],mn.prototype,`variant`,void 0),pn([D({reflect:!0})],mn.prototype,`animation`,void 0),pn([D()],mn.prototype,`width`,void 0),pn([D()],mn.prototype,`height`,void 0),mn=pn([E(`lit-material-skeleton`)],mn);var hn=o`
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
`,gn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},_n=class extends T{static{this.styles=hn}constructor(){super(),this.open=!1,this.duration=5e3,this.closable=!1,this.pauseTimer=()=>{this.open&&this.clearTimer()},this.resumeTimer=()=>{this.open&&this.startTimer()},this.handleClick=e=>{e.target.closest(`[slot="action"]`)&&this.close()},this.addEventListener(`click`,this.handleClick),this.addEventListener(`pointerenter`,this.pauseTimer),this.addEventListener(`pointerleave`,this.resumeTimer),this.addEventListener(`focusin`,this.pauseTimer),this.addEventListener(`focusout`,this.resumeTimer)}connectedCallback(){super.connectedCallback(),this.setAttribute(`popover`,`manual`),this.setAttribute(`role`,`status`),this.setAttribute(`aria-live`,`polite`),this.setAttribute(`aria-atomic`,`true`)}disconnectedCallback(){super.disconnectedCallback(),this.clearTimer()}show(e){if(e!==void 0){for(let e of[...this.childNodes])e.nodeType===Node.TEXT_NODE&&e.remove();this.prepend(document.createTextNode(e))}this.open=!0}close(){this.open=!1}updated(e){e.has(`open`)&&(this.open?(this.matches(`:popover-open`)||this.showPopover(),this.startTimer()):this.matches(`:popover-open`)&&(this.clearTimer(),this.hidePopover(),this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0}))))}render(){return x`
      <div class="content" part="snackbar">
        <slot></slot>
      </div>
      <slot name="action" class="action"></slot>
      ${this.closable?x`
            <button class="close" part="close-button" type="button" aria-label="Dismiss" @click=${this.close}>
              <svg viewBox="0 0 18 18" aria-hidden="true">
                <path d="M5 5 L13 13 M13 5 L5 13"></path>
              </svg>
            </button>
          `:w}
    `}startTimer(){this.clearTimer(),this.duration>0&&(this.timeoutId=window.setTimeout(()=>this.close(),this.duration))}clearTimer(){this.timeoutId!==void 0&&(window.clearTimeout(this.timeoutId),this.timeoutId=void 0)}};gn([D({type:Boolean,reflect:!0})],_n.prototype,`open`,void 0),gn([D({type:Number})],_n.prototype,`duration`,void 0),gn([D({type:Boolean})],_n.prototype,`closable`,void 0),_n=gn([E(`lit-material-snackbar`)],_n);var vn=o`
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
    inset-inline-start: 0;
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
    inset-inline-start: 0;
    width: 40px;
    height: 40px;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
    transition: inset-inline-start 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
  }

  .switch.checked .thumb-wrap {
    inset-inline-start: 12px;
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

  slot.icon::slotted(*) {
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
`,R=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},z=class extends T{static{this.styles=vn}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.checked=!1,this.disabled=!1,this.error=!1,this.required=!1,this.name=``,this.value=`on`,this.ariaLabel=null,this.ariaLabelledBy=null,this.ripple=new Xe(this),this.focusRing=new Ze(this),this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement),this.syncInternals()}updated(e){(e.has(`checked`)||e.has(`required`)||e.has(`error`))&&this.syncInternals()}syncInternals(){if(this.internals.setFormValue(this.checked?this.value:null),this.inputElement){let e=this.error?this.inputElement.validationMessage||`Invalid`:this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){return x`
      <div class="switch ${this.checked?`checked`:``}" part="switch">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="track" part="track">
          <div class="thumb-wrap">
            <div class="state-layer" part="ripple"></div>
            <div class="thumb" part="thumb">
              ${this.checked?x`<slot name="selected-icon" class="icon"></slot>`:x`<slot name="icon" class="icon"></slot>`}
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
          name=${this.name||w}
          value=${this.value||w}
          aria-invalid=${this.error?`true`:w}
          aria-label=${this.ariaLabel||w}
          aria-labelledby=${this.ariaLabelledBy||w}
          @change=${this.handleChange}
        />
      </div>
    `}handleChange(e){let t=e.target;this.checked=t.checked,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}formResetCallback(){this.checked=this.hasAttribute(`checked`),this.syncInternals()}formStateRestoreCallback(e){this.checked=e!==null}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};R([D({type:Boolean})],z.prototype,`checked`,void 0),R([D({type:Boolean,reflect:!0})],z.prototype,`disabled`,void 0),R([D({type:Boolean,reflect:!0})],z.prototype,`error`,void 0),R([D({type:Boolean})],z.prototype,`required`,void 0),R([D()],z.prototype,`name`,void 0),R([D()],z.prototype,`value`,void 0),R([D({reflect:!0})],z.prototype,`form`,void 0),R([D({attribute:`aria-label`})],z.prototype,`ariaLabel`,void 0),R([D({attribute:`aria-labelledby`})],z.prototype,`ariaLabelledBy`,void 0),R([k(`.native-control`)],z.prototype,`inputElement`,void 0),R([k(`.state-layer`)],z.prototype,`stateLayerElement`,void 0),z=R([E(`lit-material-switch`)],z);var yn=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
    --notch-color: var(--lit-material-text-field-notch-color, var(--md-sys-color-surface, #fffbfe));
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .text-field {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-width: var(--lit-material-text-field-min-width, 210px);
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
    border-start-start-radius: var(--lit-material-text-field-shape-start, var(--lit-material-text-field-shape, var(--md-sys-shape-corner-extra-small, 4px)));
    border-start-end-radius: var(--lit-material-text-field-shape-end, var(--lit-material-text-field-shape, var(--md-sys-shape-corner-extra-small, 4px)));
    border-end-start-radius: 0;
    border-end-end-radius: 0;
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
    border-start-start-radius: var(--lit-material-text-field-shape-start, var(--lit-material-text-field-shape, var(--md-sys-shape-corner-extra-small, 4px)));
    border-end-start-radius: var(--lit-material-text-field-shape-start, var(--lit-material-text-field-shape, var(--md-sys-shape-corner-extra-small, 4px)));
    border-start-end-radius: var(--lit-material-text-field-shape-end, var(--lit-material-text-field-shape, var(--md-sys-shape-corner-extra-small, 4px)));
    border-end-end-radius: var(--lit-material-text-field-shape-end, var(--lit-material-text-field-shape, var(--md-sys-shape-corner-extra-small, 4px)));
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
`,B=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},bn=0,V=class extends T{static{this.styles=yn}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`text`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.prefix=``,this.suffix=``,this.step=``,this.pattern=``,this.autocomplete=``,this.inputmode=``,this.name=``,this.focused=!1,this.touched=!1,this.focusRing=new Ze(this),this.labelId=`lit-material-text-field-label-${++bn}`,this.supportingId=`lit-material-text-field-supporting-${bn}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`pattern`)||e.has(`minlength`)||e.has(`maxlength`)||e.has(`min`)||e.has(`max`)||e.has(`type`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.inputElement?!1:!this.inputElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.inputElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.inputElement){let e=this.isError&&this.errorText||this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){let e=this.floated,t=this.isError;return x`
      <div class="text-field" part="text-field">
        <div
          class="container ${e?`floated`:``} ${this.focused?`focused`:``}"
          part="container"
        >
          <slot name="leading-icon" class="leading-icon"></slot>
          <div class="field">
            ${this.prefix?x`<span class="prefix">${this.prefix}</span>`:w}
            <div class="input-wrapper">
              ${this.label?x`<label class="label" part="label" id=${this.labelId} for="input">${this.label}</label>`:w}
              <input
                class="input"
                part="input"
                id="input"
                .value=${this.value}
                type=${this.type||`text`}
                placeholder=${this.placeholder||w}
                ?disabled=${this.disabled}
                ?readonly=${this.readonly}
                ?required=${this.required}
                autocomplete=${this.autocomplete||w}
                inputmode=${this.inputmode||w}
                min=${this.min??w}
                max=${this.max??w}
                minlength=${this.minlength??w}
                maxlength=${this.maxlength??w}
                step=${this.step||w}
                pattern=${this.pattern||w}
                name=${this.name||w}
                aria-labelledby=${this.label?this.labelId:w}
                aria-describedby=${this.supportingMessage?this.supportingId:w}
                aria-invalid=${t?`true`:w}
                @input=${this.handleInput}
                @change=${this.handleChange}
                @focus=${this.handleFocus}
                @blur=${this.handleBlur}
                @keydown=${this.handleKeydown}
              />
            </div>
            ${this.suffix?x`<span class="suffix">${this.suffix}</span>`:w}
          </div>
          <slot name="trailing-icon" class="trailing-icon"></slot>
        </div>
        ${this.supportingMessage||this.maxlength!=null?x`
              <div class="supporting" part="supporting">
                <span class="supporting-text" id=${this.supportingId}>
                  ${this.supportingMessage}
                </span>
                ${this.maxlength==null?w:x`<span class="counter">${this.value.length} / ${this.maxlength}</span>`}
              </div>
            `:w}
      </div>
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleKeydown(e){if(e.key!==`Enter`||e.isComposing||e.defaultPrevented)return;let t=this.internals.form;t&&(e.preventDefault(),t.requestSubmit())}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};B([D()],V.prototype,`variant`,void 0),B([D()],V.prototype,`type`,void 0),B([D()],V.prototype,`value`,void 0),B([D()],V.prototype,`label`,void 0),B([D()],V.prototype,`placeholder`,void 0),B([D({attribute:`supporting-text`})],V.prototype,`supportingText`,void 0),B([D({attribute:`error-text`})],V.prototype,`errorText`,void 0),B([D({type:Boolean,reflect:!0})],V.prototype,`error`,void 0),B([D({type:Boolean,reflect:!0})],V.prototype,`disabled`,void 0),B([D({type:Boolean})],V.prototype,`required`,void 0),B([D({type:Boolean})],V.prototype,`readonly`,void 0),B([D()],V.prototype,`prefix`,void 0),B([D()],V.prototype,`suffix`,void 0),B([D({type:Number})],V.prototype,`min`,void 0),B([D({type:Number})],V.prototype,`max`,void 0),B([D({type:Number})],V.prototype,`minlength`,void 0),B([D({type:Number})],V.prototype,`maxlength`,void 0),B([D()],V.prototype,`step`,void 0),B([D()],V.prototype,`pattern`,void 0),B([D()],V.prototype,`autocomplete`,void 0),B([D()],V.prototype,`inputmode`,void 0),B([D()],V.prototype,`name`,void 0),B([D({reflect:!0})],V.prototype,`form`,void 0),B([k(`.input`)],V.prototype,`inputElement`,void 0),B([O()],V.prototype,`focused`,void 0),B([O()],V.prototype,`touched`,void 0),V=B([E(`lit-material-text-field`)],V);var xn=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
    --notch-color: var(--lit-material-text-field-notch-color, var(--md-sys-color-surface, #fffbfe));
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .textarea-field {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-width: var(--lit-material-text-field-min-width, 210px);
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
`,H=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Sn=0,U=class extends T{static{this.styles=xn}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.rows=4,this.resize=`vertical`,this.autocomplete=``,this.name=``,this.focused=!1,this.touched=!1,this.labelId=`lit-material-textarea-label-${++Sn}`,this.supportingId=`lit-material-textarea-supporting-${Sn}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`minlength`)||e.has(`maxlength`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.textareaElement?!1:!this.textareaElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.textareaElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.textareaElement){let e=this.isError&&this.errorText||this.textareaElement.validationMessage;this.internals.setValidity(this.textareaElement.validity,e,this.textareaElement)}}render(){let e=this.floated,t=this.isError;return x`
      <div class="textarea-field" part="textarea-field">
        <div class="container ${e?`floated`:``} ${this.focused?`focused`:``}" part="container">
          ${this.label?x`<label class="label" part="label" id=${this.labelId} for="textarea">${this.label}</label>`:w}
          <textarea
            class="textarea"
            part="textarea"
            id="textarea"
            style="resize: ${this.resize}"
            rows=${this.rows}
            .value=${this.value}
            placeholder=${this.placeholder||w}
            ?disabled=${this.disabled}
            ?readonly=${this.readonly}
            ?required=${this.required}
            autocomplete=${this.autocomplete||w}
            minlength=${this.minlength??w}
            maxlength=${this.maxlength??w}
            name=${this.name||w}
            aria-labelledby=${this.label?this.labelId:w}
            aria-describedby=${this.supportingMessage?this.supportingId:w}
            aria-invalid=${t?`true`:w}
            @input=${this.handleInput}
            @change=${this.handleChange}
            @focus=${this.handleFocus}
            @blur=${this.handleBlur}
          ></textarea>
        </div>
        ${this.supportingMessage||this.maxlength!=null?x`
              <div class="supporting" part="supporting">
                <span class="supporting-text" id=${this.supportingId}> ${this.supportingMessage} </span>
                ${this.maxlength==null?w:x`<span class="counter">${this.value.length} / ${this.maxlength}</span>`}
              </div>
            `:w}
      </div>
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};H([D()],U.prototype,`variant`,void 0),H([D()],U.prototype,`value`,void 0),H([D()],U.prototype,`label`,void 0),H([D()],U.prototype,`placeholder`,void 0),H([D({attribute:`supporting-text`})],U.prototype,`supportingText`,void 0),H([D({attribute:`error-text`})],U.prototype,`errorText`,void 0),H([D({type:Boolean,reflect:!0})],U.prototype,`error`,void 0),H([D({type:Boolean,reflect:!0})],U.prototype,`disabled`,void 0),H([D({type:Boolean})],U.prototype,`required`,void 0),H([D({type:Boolean})],U.prototype,`readonly`,void 0),H([D({type:Number})],U.prototype,`minlength`,void 0),H([D({type:Number})],U.prototype,`maxlength`,void 0),H([D({type:Number})],U.prototype,`rows`,void 0),H([D()],U.prototype,`resize`,void 0),H([D()],U.prototype,`autocomplete`,void 0),H([D()],U.prototype,`name`,void 0),H([D({reflect:!0})],U.prototype,`form`,void 0),H([k(`.textarea`)],U.prototype,`textareaElement`,void 0),H([O()],U.prototype,`focused`,void 0),H([O()],U.prototype,`touched`,void 0),U=H([E(`lit-material-textarea`)],U);var Cn=o`
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
    box-sizing: border-box;
    width: 40px;
    height: 40px;
    padding: 8px;
    border: none;
    border-radius: var(--md-sys-shape-corner-full, 9999px);
    background-color: transparent;
    cursor: pointer;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    -webkit-appearance: none;
    appearance: none;
    transition:
      box-shadow 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      background-color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1)),
      color 150ms var(--md-sys-motion-easing-standard, cubic-bezier(0.2, 0, 0, 1));
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
    width: 24px;
    height: 24px;
    display: block;
    pointer-events: none;
  }

  /* Standard (default): transparent container, on-surface-variant icon. */
  :host([variant="standard"]) .button,
  :host(:not([variant])) .button {
    background-color: transparent;
    color: var(--md-sys-color-on-surface-variant, #49454f);
  }

  /* Filled: solid primary container. */
  :host([variant="filled"]) .button {
    background-color: var(--md-sys-color-primary, #6750a4);
    color: var(--md-sys-color-on-primary, #ffffff);
  }

  /* Tonal: solid secondary container, lower emphasis than filled. */
  :host([variant="tonal"]) .button {
    background-color: var(--md-sys-color-secondary-container, #e8def8);
    color: var(--md-sys-color-on-secondary-container, #1d192b);
  }

  /* Outlined: transparent container with a visible border. */
  :host([variant="outlined"]) .button {
    background-color: transparent;
    color: var(--md-sys-color-on-surface-variant, #49454f);
    border: 1px solid var(--md-sys-color-outline, #79747e);
  }

  /* Toggle "on" state: the selected icon gains primary emphasis. */
  :host([toggle][selected][variant="standard"]) .button,
  :host([toggle][selected]:not([variant])) .button {
    color: var(--md-sys-color-primary, #6750a4);
  }

  :host([toggle][selected][variant="filled"]) .button {
    background-color: var(--md-sys-color-primary, #6750a4);
    color: var(--md-sys-color-on-primary, #ffffff);
  }

  :host([toggle][selected][variant="tonal"]) .button {
    background-color: var(--md-sys-color-secondary, #625b71);
    color: var(--md-sys-color-on-secondary, #ffffff);
  }

  :host([toggle][selected][variant="outlined"]) .button {
    color: var(--md-sys-color-primary, #6750a4);
  }

  /* Disabled: icon at on-surface 38%, container transparent (or 12% for filled/tonal). */
  :host([disabled]) .button {
    color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 38%, transparent);
    box-shadow: none;
  }

  :host([variant="filled"][disabled]) .button,
  :host([variant="tonal"][disabled]) .button {
    background-color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 12%, transparent);
  }

  :host([variant="outlined"][disabled]) .button {
    background-color: transparent;
    border-color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) 12%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    .button,
    .state-layer {
      transition: none;
    }
  }
`,W=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},G=class extends T{static{this.styles=Cn}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`standard`,this.toggle=!1,this.selected=!1,this.disabled=!1,this.type=`button`,this.name=``,this.value=``,this.href=``,this.target=``,this.ariaLabel=null,this.ariaLabelledBy=null,this.ripple=new Xe(this),this.focusRing=new Ze(this),this.internals=this.attachInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}render(){let e=this.toggle&&this.selected?x`<slot name="selected-icon" class="icon"></slot>`:x`<slot name="icon" class="icon"></slot><slot class="icon"></slot>`,t=x`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      ${e}
    `;return this.href?x`
        <a
          class="button"
          part="button"
          href=${this.disabled?w:this.href}
          target=${this.target||w}
          rel=${this.target===`_blank`?`noopener noreferrer`:w}
          aria-disabled=${this.disabled?`true`:w}
          aria-label=${this.ariaLabel||w}
          aria-labelledby=${this.ariaLabelledBy||w}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${t}
        </a>
      `:x`
      <button
        class="button"
        part="button"
        type="button"
        ?disabled=${this.disabled}
        name=${this.name||w}
        value=${this.value||w}
        aria-pressed=${this.toggle?this.selected:w}
        aria-label=${this.ariaLabel||w}
        aria-labelledby=${this.ariaLabelledBy||w}
        @click=${this.handleClick}
      >
        ${t}
      </button>
    `}handleClick(e){this.disabled?(e.preventDefault(),e.stopImmediatePropagation()):this.toggle?(this.selected=!this.selected,this.dispatchEvent(new Event(`change`,{bubbles:!0}))):this.type===`submit`?this.internals.form?.requestSubmit():this.type===`reset`&&this.internals.form?.reset()}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};W([D()],G.prototype,`variant`,void 0),W([D({type:Boolean,reflect:!0})],G.prototype,`toggle`,void 0),W([D({type:Boolean,reflect:!0})],G.prototype,`selected`,void 0),W([D({type:Boolean,reflect:!0})],G.prototype,`disabled`,void 0),W([D()],G.prototype,`type`,void 0),W([D()],G.prototype,`name`,void 0),W([D()],G.prototype,`value`,void 0),W([D({reflect:!0})],G.prototype,`form`,void 0),W([D()],G.prototype,`href`,void 0),W([D()],G.prototype,`target`,void 0),W([D({attribute:`aria-label`})],G.prototype,`ariaLabel`,void 0),W([D({attribute:`aria-labelledby`})],G.prototype,`ariaLabelledBy`,void 0),W([k(`.state-layer`)],G.prototype,`stateLayerElement`,void 0),G=W([E(`lit-material-icon-button`)],G);var K=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},wn=x`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19 13H5v-2h14v2z"></path></svg>`,Tn=x`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"></path></svg>`;function En(e){let t=String(e),n=/e-(\d+)$/.exec(t);return n?Number(n[1]):t.includes(`.`)?t.length-t.indexOf(`.`)-1:0}var q=class extends T{constructor(){super(...arguments),this.value=null,this.step=1,this.label=``,this.unit=``,this.variant=`outlined`,this.supportingText=``,this.errorText=``,this.name=``,this.required=!1,this.disabled=!1,this.readonly=!1,this.validationMessage=``,this.draft=null}static{this.styles=o`
    :host {
      display: inline-flex;
      align-items: flex-start;
      gap: 4px;
      vertical-align: top;
    }
    :host([hidden]) {
      display: none;
    }
    lit-material-text-field {
      flex: 1;
      min-width: 0;
      --lit-material-text-field-min-width: var(--lit-material-number-field-width, 8ch);
    }
    lit-material-icon-button {
      margin-block-start: 8px;
    }
    svg {
      width: 100%;
      height: 100%;
      fill: currentColor;
    }
  `}checkValidity(){return this.validationMessage=this.check(),!this.validationMessage}check(){return this.draft!==null&&this.draft.trim()!==``?`Enter a number`:this.value===null?this.required?`Required`:``:this.min!==void 0&&this.value<this.min?`At least ${this.min}`:this.max!==void 0&&this.value>this.max?`At most ${this.max}`:``}willUpdate(e){e.has(`value`)&&(this.draft=null),this.validationMessage=this.check()}clamp(e){return this.min!==void 0&&(e=Math.max(this.min,e)),this.max!==void 0&&(e=Math.min(this.max,e)),e}stepBy(e){if(this.value===null){this.commit(this.clamp(this.min??0));return}let t=this.value,n=Math.max(En(this.step),En(t));this.commit(this.clamp(Number((t+e*this.step).toFixed(n))))}commit(e){let t=e!==this.value||this.draft!==null;this.value=e,this.draft=null,t&&this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0}))}onInput(e){e.stopPropagation();let t=e.target.value.trim(),n=t===``?null:Number(t.replace(`,`,`.`));n!==null&&!Number.isFinite(n)?(this.draft=t,this.validationMessage=this.check()):(this.value=n,this.draft=null,this.dispatchEvent(new Event(`input`,{bubbles:!0,composed:!0})))}onKeyDown(e){if(this.disabled||this.readonly)return;let t={ArrowUp:1,ArrowDown:-1,PageUp:10,PageDown:-10};e.key in t?(e.preventDefault(),this.stepBy(t[e.key])):e.key===`Home`&&this.min!==void 0?(e.preventDefault(),this.commit(this.min)):e.key===`End`&&this.max!==void 0&&(e.preventDefault(),this.commit(this.max))}render(){let e=this.disabled||this.readonly,t=this.label||`value`,n=this.validationMessage||this.errorText,r=this.draft??(this.value===null?``:String(this.value));return x`
      <lit-material-icon-button
        part="decrement"
        aria-label=${`Decrease ${t}`}
        ?disabled=${e||this.min!==void 0&&this.value!==null&&this.value<=this.min}
        @click=${()=>this.stepBy(-1)}
        ><span slot="icon">${wn}</span></lit-material-icon-button
      >
      <lit-material-text-field
        part="field"
        variant=${this.variant}
        label=${this.label}
        inputmode=${this.step%1==0&&(this.min??0)>=0?`numeric`:`decimal`}
        autocomplete="off"
        .value=${r}
        suffix=${this.unit||w}
        name=${this.name||w}
        supporting-text=${this.supportingText}
        error-text=${n}
        ?error=${!!n}
        ?required=${this.required}
        ?disabled=${this.disabled}
        ?readonly=${this.readonly}
        @input=${this.onInput}
        @change=${e=>{e.stopPropagation(),this.draft===null&&this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0}))}}
        @keydown=${this.onKeyDown}
      ></lit-material-text-field>
      <lit-material-icon-button
        part="increment"
        aria-label=${`Increase ${t}`}
        ?disabled=${e||this.max!==void 0&&this.value!==null&&this.value>=this.max}
        @click=${()=>this.stepBy(1)}
        ><span slot="icon">${Tn}</span></lit-material-icon-button
      >
    `}};K([D({type:Number})],q.prototype,`value`,void 0),K([D({type:Number})],q.prototype,`min`,void 0),K([D({type:Number})],q.prototype,`max`,void 0),K([D({type:Number})],q.prototype,`step`,void 0),K([D()],q.prototype,`label`,void 0),K([D()],q.prototype,`unit`,void 0),K([D()],q.prototype,`variant`,void 0),K([D({attribute:`supporting-text`})],q.prototype,`supportingText`,void 0),K([D({attribute:`error-text`})],q.prototype,`errorText`,void 0),K([D()],q.prototype,`name`,void 0),K([D({type:Boolean})],q.prototype,`required`,void 0),K([D({type:Boolean,reflect:!0})],q.prototype,`disabled`,void 0),K([D({type:Boolean})],q.prototype,`readonly`,void 0),K([O()],q.prototype,`validationMessage`,void 0),K([O()],q.prototype,`draft`,void 0),q=K([E(`lit-material-number-field`)],q);var Dn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},On=class extends T{static{this.styles=o`
    :host {
      display: flex;
      align-items: flex-start;
    }
    :host([hidden]) {
      display: none;
    }
    ::slotted(*) {
      position: relative;
      /* Buttons take the fields' height and corners. */
      --lit-material-button-height: 56px;
      --lit-material-button-shape: var(--md-sys-shape-corner-extra-small, 4px);
      --lit-material-select-min-width: 0;
    }
    ::slotted(lit-material-text-field) {
      flex: 1;
      min-width: 0;
      --lit-material-text-field-min-width: 0;
    }
    ::slotted(:not(:first-child)) {
      margin-inline-start: -1px;
      --lit-material-text-field-shape-start: 0;
      --lit-material-select-shape-start: 0;
      --lit-material-button-shape-start: 0;
    }
    ::slotted(:not(:last-child)) {
      --lit-material-text-field-shape-end: 0;
      --lit-material-select-shape-end: 0;
      --lit-material-button-shape-end: 0;
    }
    /* The focused piece's thicker outline goes over its neighbour's edge. */
    ::slotted(:focus-within) {
      z-index: 1;
    }
  `}willUpdate(){this.hasAttribute(`role`)||this.setAttribute(`role`,`group`)}render(){return x`<slot></slot>`}};On=Dn([E(`lit-material-input-group`)],On);var kn=`rws-admin-token`,An=``;function jn(){try{return sessionStorage.getItem(kn)??An}catch{return An}}function Mn(e){An=e;try{e?sessionStorage.setItem(kn,e):sessionStorage.removeItem(kn)}catch{}}var J=class extends Error{constructor(e,t){super(t),this.status=e}};async function Y(e,{method:t=`GET`,body:n}={}){let r=await fetch(`/admin/api/v1${e}`,{method:t,headers:{...jn()?{Authorization:`Bearer ${jn()}`}:{},"X-Rws-Admin":`1`,...n===void 0?{}:{"Content-Type":`application/json`}},credentials:`same-origin`,body:n===void 0?void 0:JSON.stringify(n),cache:`no-store`}),i=await r.text(),a=null;try{a=i?JSON.parse(i):null}catch{a=null}if(!r.ok)throw new J(r.status,a?.error??`${r.status} ${r.statusText}`);return a}async function Nn(e,t){let n=await fetch(`/admin/api/v1/logs/stream`,{headers:{...jn()?{Authorization:`Bearer ${jn()}`}:{},"X-Rws-Admin":`1`},credentials:`same-origin`,signal:t,cache:`no-store`});if(!n.ok||!n.body)throw new J(n.status,`${n.status} ${n.statusText}`);let r=n.body.pipeThrough(new TextDecoderStream).getReader(),i=``;for(;;){let{value:t,done:n}=await r.read();if(n)return;i+=t;let a;for(;(a=i.indexOf(`

`))>=0;){let t=i.slice(0,a);if(i=i.slice(a+2),t.split(`
`).some(e=>e.startsWith(`event:`)))continue;let n=t.split(`
`).filter(e=>e.startsWith(`data:`)).map(e=>e.slice(5).replace(/^ /,``)).join(`
`);n&&e(n)}}}var Pn=[`B`,`KiB`,`MiB`,`GiB`,`TiB`,`PiB`,`EiB`],Fn=[`B`,`kB`,`MB`,`GB`,`TB`,`PB`,`EB`];function In(e,t={}){let{locale:n,maximumFractionDigits:r=1,...i}=t;return new Intl.NumberFormat(n,{maximumFractionDigits:r,...i}).format(e)}function Ln(e,t={}){if(!Number.isFinite(e))return``;let n=t.binary===!1?1e3:1024,r=t.binary===!1?Fn:Pn,i=Math.abs(e),a=0;for(;i>=n&&a<r.length-1;)i/=n,a++;let o=t.maximumFractionDigits??1;return a<r.length-1&&Number(i.toFixed(o))>=n&&(i/=n,a++),`${e<0?`-`:``}${In(i,{...t,maximumFractionDigits:a===0?0:o})} ${r[a]}`}var Rn=[[`day`,864e5],[`hour`,36e5],[`minute`,6e4],[`second`,1e3],[`millisecond`,1]];function zn(e,t={}){if(!Number.isFinite(e))return``;let{locale:n,largest:r=2,unitDisplay:i=`short`}=t,a=Math.abs(Math.round(e)),o=[];for(let[t,s]of Rn){if(t===`millisecond`&&(o.length||Math.abs(e)>=1e3))break;let c=Math.floor(a/s);if(a-=c*s,c>0&&o.push(new Intl.NumberFormat(n,{style:`unit`,unit:t,unitDisplay:i}).format(c)),o.length===r)break}o.length===0&&o.push(new Intl.NumberFormat(n,{style:`unit`,unit:`second`,unitDisplay:i}).format(0));let s=new Intl.ListFormat(n,{style:i,type:`unit`}).format(o);return e<0?`-${s}`:s}var Bn=`0 0 1000 439`,Vn={AE:`M637,154L638,154L638,155L640,154L642,154L644,154L645,152L647,150L648,148L649,149L650,152L648,152L648,154L649,154L648,155L648,157L648,158L648,160L647,160L639,159L637,155L637,154Z`,AF:`M666,109L668,109L670,110L671,110L672,109L673,110L674,108L675,108L675,106L676,105L678,106L678,107L679,110L680,111L681,110L682,110L683,108L685,109L687,109L688,110L687,110L686,111L683,111L681,112L680,113L681,115L681,116L681,118L681,119L681,120L678,120L680,122L678,123L678,125L679,127L678,128L677,128L675,128L675,129L673,129L672,131L672,134L669,136L668,135L667,136L666,136L663,136L658,134L660,131L659,129L657,128L657,126L655,124L656,122L654,121L655,119L655,115L657,116L659,115L659,114L661,113L662,112L662,110L664,109L664,108L666,109Z`,AL:`M551,98L552,98L551,99L551,100L550,101L549,101L548,99L547,98L547,96L548,95L547,94L547,93L548,92L549,92L549,93L550,93L550,94L550,95L550,97L551,98Z`,AM:`M615,104L614,104L613,102L612,102L611,101L610,101L609,100L607,99L607,98L607,97L610,96L611,97L612,98L611,98L613,100L612,101L614,101L615,102L615,104Z`,AO:`M536,261L535,262L535,263L534,265L533,262L534,261L535,260L536,261ZM534,266L535,266L536,266L537,265L545,265L546,268L547,270L547,272L548,274L550,273L551,273L553,273L553,272L554,270L555,270L556,269L557,269L557,271L560,271L560,273L561,274L560,277L560,279L561,280L561,285L562,284L563,284L565,284L566,284L566,285L566,287L566,289L566,290L566,291L560,291L560,303L561,306L563,308L558,310L551,309L550,308L539,308L538,308L537,306L535,306L533,307L532,308L532,305L532,302L533,299L533,297L534,294L535,292L537,290L537,288L538,285L538,283L537,282L536,280L536,278L536,277L537,275L536,272L535,269L534,267L534,266Z`,AR:`M346,425L348,426L350,428L353,430L357,431L357,432L354,432L353,431L352,431L349,431L346,425ZM350,354L350,356L350,360L351,363L351,364L351,366L351,368L355,371L355,373L357,375L358,376L356,381L353,382L348,383L345,383L347,385L347,387L348,389L347,390L344,390L341,389L341,390L342,393L344,394L345,393L346,395L344,396L343,398L344,401L344,402L342,402L340,404L341,406L344,408L346,409L347,412L344,413L344,417L343,418L343,419L345,422L347,424L346,424L344,423L338,423L336,421L335,419L334,419L333,418L331,415L332,414L332,412L331,411L331,408L330,404L329,402L330,402L329,400L328,400L328,399L327,397L325,394L326,393L324,389L323,386L323,383L324,382L322,379L321,375L323,373L322,371L322,367L321,364L318,358L319,354L318,351L318,348L319,344L320,342L319,341L319,340L318,334L321,332L321,328L321,327L322,324L326,325L328,327L328,325L332,325L332,326L338,331L340,332L344,335L347,336L347,337L346,343L349,344L352,344L354,344L356,341L356,338L357,337L358,339L359,342L357,344L355,346L353,349L350,354Z`,AT:`M539,75L539,76L538,76L539,77L538,79L536,79L534,80L533,80L529,79L528,78L526,79L525,79L523,79L522,78L523,77L522,77L523,77L524,77L525,77L527,77L528,76L529,76L530,77L530,76L530,74L531,74L531,73L533,74L534,72L535,72L537,73L538,73L539,74L539,75Z`,AU:`M862,389L863,389L860,393L859,394L857,397L856,396L853,398L851,398L852,395L853,393L854,390L855,389L856,389L858,390L859,390L862,389ZM824,361L821,362L818,363L817,365L815,366L813,366L811,367L809,366L807,366L806,367L803,368L801,369L799,370L797,370L796,370L794,368L793,367L794,365L795,365L796,364L796,363L798,360L798,358L798,355L799,353L800,351L799,349L800,348L799,346L800,344L799,341L799,339L800,341L800,338L801,339L802,340L802,338L802,335L802,334L801,333L802,331L803,330L804,328L804,326L806,324L806,326L807,324L810,323L812,321L814,320L815,320L816,320L819,319L820,318L821,318L822,317L823,317L826,316L828,315L829,313L831,311L832,309L832,307L835,304L835,307L837,307L836,305L837,303L838,304L839,301L841,299L842,298L844,297L844,296L845,297L845,296L847,295L848,295L850,296L851,299L853,299L854,299L854,297L856,294L857,293L857,292L859,290L861,289L862,289L864,289L865,287L863,285L864,285L866,286L867,287L869,288L870,288L872,289L874,288L875,288L876,289L875,291L874,293L873,293L873,294L872,296L870,298L870,299L872,301L874,302L875,304L876,306L877,306L878,307L878,308L881,309L883,308L884,306L885,304L886,302L887,299L887,298L888,296L888,294L889,292L889,291L889,290L890,288L891,286L891,285L893,283L893,285L893,287L893,288L893,289L894,291L894,293L893,295L894,297L896,296L896,298L897,299L897,301L896,304L896,306L897,306L897,309L896,311L896,314L898,316L900,317L901,319L900,320L901,322L901,326L902,325L903,327L904,326L903,330L904,332L904,333L905,336L905,339L904,341L903,343L902,346L901,349L899,351L897,354L896,356L894,359L891,362L888,363L886,366L884,368L882,371L880,372L877,375L876,377L875,378L873,379L870,379L867,381L865,382L862,383L861,382L860,381L862,380L860,380L856,383L855,382L854,381L852,381L850,380L850,378L851,375L852,374L851,372L849,372L851,370L851,368L849,370L846,371L849,369L850,367L852,365L853,363L850,366L847,367L845,370L844,368L845,366L844,364L844,363L845,362L842,360L841,360L839,358L834,358L830,360L827,361L824,361Z`,AZ:`M613,94L614,94L616,96L617,97L617,96L618,94L620,96L622,98L623,99L624,99L622,100L622,102L622,103L621,104L622,106L621,106L619,104L620,103L619,102L618,102L615,104L615,102L614,101L612,101L613,100L611,98L612,98L611,97L610,96L612,97L614,97L614,96L612,95L613,94ZM614,104L613,104L611,102L610,101L611,101L612,102L613,102L614,104Z`,BA:`M545,92L543,91L542,89L541,89L539,87L539,86L538,85L538,84L539,84L540,84L543,84L544,84L545,85L546,85L546,86L547,87L547,89L546,89L545,90L545,92Z`,BD:`M748,162L749,165L748,164L749,167L747,165L747,163L746,161L744,159L742,159L743,161L742,163L741,162L741,163L740,162L739,162L738,159L736,156L736,154L734,153L734,152L736,151L733,149L734,146L736,148L737,148L738,150L741,151L743,151L745,151L745,154L743,155L743,157L745,159L745,156L746,156L748,162Z`,BE:`M514,67L514,69L513,69L513,71L511,69L510,70L508,68L507,67L506,67L506,66L508,66L509,66L511,65L513,66L514,67Z`,BF:`M485,205L485,203L486,201L486,200L488,197L488,194L489,193L490,194L491,193L492,192L494,190L495,189L497,188L499,187L499,188L501,188L501,190L501,191L503,194L503,195L506,196L506,199L505,200L504,200L503,202L502,202L500,202L499,202L498,203L497,202L492,202L492,205L492,207L490,206L489,207L488,207L487,207L486,205L485,205Z`,BG:`M554,87L555,88L556,88L558,88L561,89L563,88L565,87L567,88L569,88L568,90L567,92L568,94L566,93L564,94L564,96L562,96L560,95L558,96L556,96L556,94L554,93L555,92L554,92L555,91L555,90L554,89L554,88L554,87Z`,BI:`M585,252L585,254L585,255L585,256L585,257L584,259L583,260L581,260L581,256L581,254L582,254L583,252L585,252Z`,BJ:`M507,220L505,220L504,218L505,209L504,209L504,207L503,205L502,204L502,202L503,202L504,200L505,200L506,199L507,198L508,198L510,200L510,201L510,203L510,205L510,206L509,208L508,209L508,212L508,214L507,220Z`,BN:`M820,223L820,225L820,227L819,227L818,228L817,226L818,225L820,223Z`,BO:`M309,284L311,285L312,284L313,283L315,282L316,280L320,280L320,283L320,284L320,286L323,290L326,290L328,292L330,293L331,294L333,294L334,295L335,297L335,298L335,299L335,300L336,304L341,304L341,306L342,307L343,309L344,311L344,314L343,315L344,317L343,318L343,317L340,315L338,315L333,316L332,319L333,321L332,326L332,325L328,325L328,327L326,325L322,324L321,327L319,328L317,323L315,319L315,315L313,314L313,311L311,309L312,305L311,302L311,300L311,299L311,297L311,294L311,291L311,290L309,284Z`,BR:`M364,366L363,364L364,362L362,360L359,358L356,356L355,356L352,353L350,354L353,349L355,346L357,344L359,342L358,339L357,337L356,338L356,336L356,334L355,332L354,331L353,332L352,332L352,330L351,327L351,326L349,325L348,326L345,325L344,320L343,318L344,317L343,315L344,314L344,311L343,309L342,307L341,306L341,304L336,304L335,300L335,299L335,298L335,297L334,295L333,294L331,294L330,293L328,292L326,290L323,290L320,286L320,284L320,283L320,280L316,280L315,282L313,283L312,284L311,285L309,284L307,285L306,284L305,279L303,281L301,281L300,279L298,279L298,277L297,275L295,271L296,271L296,269L298,268L297,266L298,265L298,263L301,261L303,260L304,259L306,259L307,249L307,248L307,245L305,244L306,241L307,241L308,241L308,240L306,239L306,237L312,237L312,236L313,237L314,239L316,241L318,240L318,239L321,238L322,238L322,236L324,235L324,234L322,234L321,232L321,229L320,228L321,228L323,228L325,229L326,228L328,228L331,226L332,225L332,224L333,224L334,225L333,226L334,227L335,229L334,230L333,233L334,235L334,237L336,238L337,239L338,238L339,238L340,237L341,236L342,236L343,236L344,237L345,236L344,235L345,234L346,234L347,234L349,235L350,235L351,234L351,235L352,236L353,235L354,234L355,231L357,228L358,228L358,230L360,236L361,237L361,239L359,243L360,244L365,244L365,248L367,246L371,247L375,249L377,251L376,253L379,252L385,254L389,254L393,257L397,261L399,262L401,263L402,264L403,269L404,271L403,277L402,279L398,285L396,289L395,292L394,292L393,295L394,302L394,307L393,310L393,311L393,316L390,321L390,324L388,326L388,328L385,328L381,329L379,331L376,332L374,335L372,338L372,341L373,343L373,347L372,348L371,350L369,356L367,359L366,361L365,364L364,366Z`,BS:`M292,145L293,145L295,145L295,146L292,146L292,145ZM295,144L297,146L296,148L296,146L295,145L295,144ZM293,151L294,151L294,154L293,156L292,154L292,153L293,151Z`,BT:`M741,142L742,143L742,145L740,145L738,145L736,145L734,144L733,143L734,141L736,140L738,141L739,141L741,142Z`,BW:`M579,325L575,328L572,330L571,333L570,334L569,334L568,336L568,337L566,338L564,338L563,337L562,336L560,337L560,339L558,340L557,341L555,342L555,341L555,338L554,335L553,335L553,324L556,324L557,311L559,311L563,310L564,311L566,310L567,310L568,309L569,309L569,312L570,313L571,315L574,319L575,319L575,320L575,323L577,323L579,325Z`,BY:`M561,52L564,53L564,54L565,53L567,54L568,55L568,56L569,58L570,58L570,59L572,59L573,60L572,60L570,60L570,61L571,62L572,63L570,64L569,64L569,66L568,65L566,65L565,65L564,65L562,65L559,64L557,64L555,64L554,65L553,65L553,64L552,62L553,62L553,61L552,60L552,58L554,58L556,57L557,56L558,55L558,54L559,53L561,52Z`,BZ:`M258,177L259,177L260,175L261,175L261,176L260,178L260,180L259,182L259,183L258,183L257,184L258,180L258,177Z`,CA:`M216,72L214,69L213,68L210,67L211,64L213,63L211,62L213,59L212,57L213,56L215,55L217,53L215,51L216,48L217,46L216,45L216,44L216,43L213,44L209,45L209,43L209,42L208,42L206,42L223,28L233,21L236,21L238,22L240,22L242,22L246,21L248,21L252,20L256,20L256,21L258,20L260,19L261,20L262,21L266,20L264,21L267,21L268,21L271,21L272,21L276,22L278,23L280,22L282,23L278,24L281,25L286,25L288,24L289,25L292,24L291,24L293,23L296,23L297,23L298,23L299,24L301,24L304,25L307,25L310,25L311,23L313,23L315,24L314,26L316,24L318,24L321,22L320,21L318,20L321,18L324,17L326,17L327,18L328,20L325,21L328,21L327,23L330,22L332,23L330,24L330,26L334,24L337,23L339,21L341,21L344,21L345,22L345,23L342,24L343,25L342,26L337,28L334,28L332,27L331,28L327,30L326,31L322,33L319,33L317,34L316,35L313,35L309,37L305,40L302,42L300,45L303,45L302,48L301,50L305,49L308,50L309,51L310,53L312,53L314,55L317,55L320,55L318,57L317,60L317,63L319,66L321,65L324,62L325,58L324,56L329,55L332,53L334,51L335,49L335,47L334,45L338,43L338,40L340,37L342,36L345,37L347,37L349,36L351,37L352,39L352,40L356,40L355,42L354,45L355,45L356,47L360,45L363,43L365,41L366,44L367,47L368,50L366,52L368,53L370,55L373,55L374,56L374,59L375,59L376,60L375,63L373,64L371,65L367,66L363,69L359,69L354,69L351,69L348,69L345,71L342,72L337,76L333,79L335,78L341,74L346,72L350,72L351,73L348,75L348,78L348,81L350,82L354,82L358,78L357,81L359,82L355,84L349,85L346,87L343,89L341,89L342,86L347,84L343,84L340,84L339,82L341,78L340,77L339,78L338,77L335,79L334,82L332,83L331,84L330,84L325,84L322,84L320,85L317,87L315,89L313,89L311,89L309,89L310,90L309,91L305,93L303,93L299,95L298,94L299,92L301,91L302,89L303,86L304,83L302,82L303,81L302,81L302,80L301,80L301,79L299,78L298,77L296,76L295,74L292,75L291,75L289,75L287,75L285,74L283,74L282,73L281,73L282,71L281,71L280,72L275,72L268,72L260,72L253,72L246,72L239,72L232,72L230,72L223,72L216,72ZM329,36L331,35L334,35L333,36L330,37L329,37L329,36ZM354,15L353,14L354,14L358,14L361,15L360,15L359,15L357,15L354,15ZM335,37L337,37L338,37L336,38L335,38L335,37ZM332,12L331,12L328,12L326,12L328,11L331,11L332,11L332,12ZM336,8L335,8L332,8L335,8L336,8ZM333,7L334,7L333,8L330,8L329,8L329,7L332,7L333,7ZM341,13L338,13L334,12L334,11L335,11L334,10L330,10L329,9L330,9L334,9L335,9L339,9L340,10L339,10L340,11L341,11L344,11L346,11L349,11L353,11L356,11L357,11L357,12L355,12L352,13L350,12L345,13L341,13ZM306,7L309,8L308,8L304,8L302,8L304,8L306,7ZM308,7L310,7L307,7L305,7L307,7L308,7ZM374,66L372,67L370,70L372,69L373,70L372,71L373,71L375,71L377,72L375,74L377,73L377,75L377,77L375,79L374,79L373,79L374,76L373,76L370,79L369,79L371,77L369,76L366,77L362,77L362,76L363,75L363,74L365,72L369,67L371,66L374,65L375,65L374,66ZM333,30L335,31L337,32L336,33L338,33L339,33L336,34L334,34L333,33L330,34L326,35L327,34L324,34L327,33L328,31L331,29L332,29L331,30L333,30ZM355,16L357,15L361,16L363,17L362,18L366,17L367,19L371,19L372,20L373,22L369,23L372,24L375,25L376,26L379,27L378,28L373,31L371,30L369,28L367,28L366,29L367,30L369,31L369,32L369,34L368,36L365,35L361,33L363,35L364,37L364,38L359,37L356,35L354,34L355,34L353,32L351,31L350,32L344,32L343,32L346,30L349,30L354,30L354,29L355,28L359,26L359,25L359,24L357,23L353,22L355,22L354,21L352,21L351,20L350,21L346,21L340,20L336,20L334,20L333,19L335,18L333,18L334,16L337,15L339,14L344,14L342,15L343,16L346,14L351,14L352,15L351,17L355,16ZM329,13L333,13L336,14L332,15L329,15L326,17L324,17L324,15L325,14L327,14L329,13ZM282,10L287,9L292,8L294,8L297,8L295,9L293,10L292,10L287,10L284,10L282,10ZM206,58L209,58L205,61L205,63L204,63L204,62L204,61L204,60L205,58L206,58ZM318,6L321,6L326,7L326,8L324,8L321,7L317,7L319,7L318,7L318,6ZM214,74L212,74L209,73L209,72L208,71L208,70L207,69L207,68L208,67L210,68L211,68L213,69L213,70L213,71L214,72L214,74ZM281,13L283,13L288,13L289,14L290,14L287,15L281,16L277,17L276,18L270,19L270,18L267,17L269,16L272,15L275,14L275,13L281,13ZM308,11L310,10L312,10L312,11L310,12L303,12L298,13L295,13L296,12L300,12L292,12L290,11L294,10L296,10L301,10L303,11L306,11L305,10L307,9L309,9L308,10L308,11ZM306,15L307,16L306,17L305,18L307,19L310,20L309,21L305,21L306,22L304,23L301,22L299,22L296,22L292,23L287,23L283,23L283,22L281,22L279,22L279,20L280,20L284,20L286,20L289,20L286,19L282,19L279,19L284,18L281,18L279,17L282,16L284,15L290,14L292,15L290,16L294,15L296,16L299,15L300,16L300,17L301,17L302,15L304,15L306,15ZM316,15L315,14L318,14L320,14L323,14L321,15L323,16L321,17L317,18L315,18L315,17L312,16L312,15L316,15ZM306,14L309,14L310,14L307,15L306,14ZM326,9L327,10L326,11L324,12L321,12L319,12L320,11L317,11L319,10L320,10L324,9L326,10L326,9ZM336,5L338,4L340,4L339,4L343,4L345,5L347,5L350,5L350,6L352,6L349,6L345,7L342,7L339,7L337,7L338,6L340,6L336,6L335,6L334,5L336,5ZM345,4L348,3L350,3L353,3L356,3L358,3L360,3L361,3L364,3L367,3L372,3L373,3L378,2L381,3L385,3L389,3L393,3L396,3L391,4L387,4L385,4L389,4L384,4L381,5L378,6L374,6L373,6L367,6L370,6L368,6L369,7L367,7L364,8L363,8L360,9L363,9L357,10L353,10L348,10L345,10L342,10L342,9L346,9L346,8L347,8L351,8L350,7L347,7L349,7L352,6L353,6L351,6L351,5L356,5L357,5L360,5L356,5L350,5L348,5L347,4L345,4ZM354,25L353,26L350,26L351,25L352,24L354,24L355,24L355,25L354,25ZM318,21L318,22L316,23L314,22L312,22L311,21L313,21L315,20L317,21L318,21ZM352,70L353,70L356,70L357,72L356,72L354,71L352,70ZM350,78L350,80L352,80L354,80L352,81L351,81L349,80L349,79L350,78Z`,CD:`M581,260L582,264L582,266L582,268L584,270L585,275L584,274L580,275L579,275L579,278L579,279L578,284L578,287L579,288L581,289L581,293L579,293L578,291L577,289L575,289L575,287L573,288L571,287L570,286L568,285L567,285L567,284L566,284L565,284L563,284L562,284L561,285L561,280L560,279L560,277L561,274L560,273L560,271L557,271L557,269L556,269L555,270L554,270L553,272L553,273L551,273L550,273L548,274L547,272L547,270L546,268L545,265L537,265L536,266L535,266L534,266L534,265L535,263L535,262L536,261L537,262L538,260L539,260L539,261L540,262L542,260L544,258L544,257L544,254L546,250L547,248L549,246L549,245L549,244L550,242L549,240L550,237L550,235L551,233L551,230L551,228L553,226L554,225L556,226L558,227L560,228L562,228L563,226L565,226L568,224L569,225L570,225L570,224L571,224L573,224L575,224L576,224L578,227L579,227L580,227L581,227L582,226L583,228L586,230L585,235L587,235L586,236L585,237L584,239L583,241L583,244L582,246L582,248L581,249L581,252L581,254L581,256L581,260Z`,CF:`M576,224L575,224L573,224L571,224L570,224L570,225L569,225L568,224L565,226L563,226L562,228L560,228L558,227L556,226L554,225L553,226L551,228L551,230L549,230L548,229L546,231L544,235L544,234L544,232L543,231L542,229L541,228L540,226L540,225L540,223L540,220L541,219L542,216L545,215L545,214L546,214L546,215L550,214L551,212L552,211L552,210L553,209L555,210L558,208L560,204L561,202L563,202L563,203L565,206L565,207L565,209L565,210L566,211L568,213L569,214L570,215L571,217L573,219L573,221L575,223L576,224Z`,CG:`M551,230L551,233L550,235L550,237L549,240L550,242L549,244L549,245L549,246L547,248L546,250L544,254L544,257L544,258L542,260L540,262L539,261L539,260L538,260L537,262L536,261L535,260L534,261L533,262L531,258L533,256L532,254L533,253L535,252L535,251L536,252L539,253L540,251L540,248L540,245L538,243L540,239L539,238L537,238L536,237L536,235L540,235L542,236L544,237L544,235L546,231L548,229L549,230L551,230Z`,CH:`M522,77L523,77L522,78L523,79L525,79L524,80L523,80L522,80L521,81L520,81L518,82L517,82L516,81L515,80L514,81L514,79L516,77L517,77L517,76L519,76L520,76L522,77Z`,CI:`M478,205L479,205L481,206L482,204L482,205L483,204L483,206L484,205L485,205L486,205L487,207L488,207L489,207L490,206L492,207L493,213L492,216L491,220L492,223L492,225L491,225L489,224L487,224L484,225L482,226L479,227L479,224L479,223L479,222L478,220L477,220L476,219L477,218L477,216L477,215L477,213L477,212L478,211L478,208L477,207L477,206L478,205Z`,CL:`M346,425L349,431L352,431L353,431L353,432L351,433L350,433L349,433L347,432L344,432L341,430L338,429L333,425L335,426L339,428L343,429L343,428L343,426L344,425L346,425ZM311,309L313,311L313,314L315,315L315,319L317,323L319,328L321,327L321,328L321,332L318,334L319,340L319,341L320,342L319,344L318,348L318,351L319,354L318,358L321,364L322,367L322,371L323,373L321,375L322,379L324,382L323,383L323,386L324,389L326,393L325,394L327,397L328,399L328,400L329,400L330,402L329,402L330,404L331,408L331,411L332,412L332,414L331,415L333,418L334,419L335,419L336,421L338,423L344,423L346,424L344,424L343,425L342,426L343,428L342,428L339,427L335,425L331,424L330,422L330,420L328,418L325,413L325,411L326,408L322,407L323,405L322,399L325,401L324,394L322,393L323,397L321,397L320,392L319,386L319,384L317,381L316,377L317,377L317,372L317,366L317,361L315,356L315,353L313,349L314,345L313,338L312,330L312,322L311,317L309,311L311,310L311,309Z`,CM:`M540,195L541,198L541,200L541,203L543,206L541,206L540,206L539,206L539,208L540,210L541,211L542,212L543,215L542,216L541,219L540,220L540,223L540,225L540,226L541,228L542,229L543,231L544,232L544,234L544,235L544,237L542,236L540,235L536,235L534,235L533,235L531,235L527,235L527,232L526,229L525,229L524,227L524,226L524,223L526,219L528,217L529,217L531,219L533,217L533,216L533,214L534,212L535,211L536,208L536,207L537,205L537,203L540,200L540,199L540,198L539,197L539,196L540,195Z`,CN:`M797,176L794,175L794,172L794,170L797,169L799,169L800,170L799,172L799,174L797,176ZM695,93L694,91L695,90L690,85L695,83L696,83L695,77L700,78L700,77L699,74L701,74L701,72L702,71L704,74L707,75L711,76L714,79L715,82L716,84L719,84L723,85L726,86L728,87L731,90L733,92L736,91L741,92L744,92L747,92L751,94L754,94L756,95L758,93L761,92L765,92L767,91L768,89L769,88L768,87L766,86L766,84L768,84L770,85L772,83L775,82L775,80L776,79L779,79L781,79L781,78L777,76L775,75L774,76L771,76L770,76L769,75L768,72L768,70L771,71L773,69L772,68L772,65L772,64L771,62L769,62L770,60L772,60L775,60L779,60L782,61L786,64L788,66L790,67L793,70L798,71L802,73L805,76L809,76L810,75L813,74L814,77L814,78L816,81L817,84L813,83L812,85L815,87L817,91L816,91L817,93L814,91L814,92L811,94L813,96L811,95L809,94L808,97L807,98L806,100L803,101L802,103L800,104L801,102L799,101L800,99L798,97L796,98L795,101L794,103L792,103L792,104L794,107L796,107L797,109L800,110L801,107L804,108L806,109L807,110L804,111L804,113L802,115L802,117L806,119L808,122L811,125L814,128L815,131L814,131L815,133L817,134L818,137L819,140L817,140L817,144L816,149L815,153L813,156L810,159L807,160L806,161L805,160L804,162L800,164L798,164L798,168L796,168L795,166L795,164L791,163L790,164L787,163L785,161L785,159L783,159L781,157L779,159L777,160L775,160L773,160L772,161L774,165L772,165L772,164L771,163L770,164L768,163L766,162L766,159L764,158L763,155L760,155L759,151L761,148L760,145L759,143L758,142L756,140L755,140L752,139L752,138L750,136L749,137L747,136L744,139L743,141L741,142L739,141L738,141L736,140L734,141L733,143L732,140L731,141L728,141L725,140L722,139L720,138L719,136L717,136L714,133L711,132L711,133L706,131L703,129L701,125L703,125L703,124L701,122L701,119L697,115L692,114L691,111L688,110L687,109L686,107L686,106L684,105L682,102L682,101L684,99L685,98L688,99L688,97L691,96L691,95L695,93Z`,CO:`M314,239L313,237L312,236L312,237L306,237L306,239L308,240L308,241L307,241L306,241L305,244L307,245L307,248L307,249L306,259L305,257L304,257L306,254L303,252L302,252L301,251L299,252L297,252L295,248L294,247L293,245L291,244L289,243L288,242L287,242L285,242L284,240L281,238L281,237L282,237L282,235L282,234L284,233L285,231L286,229L285,228L286,226L285,222L286,222L285,218L284,216L285,215L286,215L286,214L286,212L286,211L288,211L290,208L291,208L291,207L292,204L294,202L296,202L296,201L298,201L300,200L301,199L303,197L304,197L305,198L304,199L302,200L301,202L300,203L299,204L299,207L298,209L299,209L299,211L300,212L300,213L300,215L300,216L301,217L304,217L306,217L308,221L309,220L311,220L312,220L313,221L313,223L312,224L312,227L312,229L313,230L313,231L312,233L313,234L313,235L314,239Z`,CR:`M272,208L271,208L271,209L272,210L271,210L271,211L271,212L271,213L269,212L269,211L269,210L268,209L267,208L266,207L266,206L266,207L265,208L265,207L264,206L263,206L264,204L264,203L263,203L264,202L265,202L266,203L267,202L268,202L268,203L269,203L270,203L270,205L271,206L272,208Z`,CU:`M280,158L283,158L285,158L287,159L288,161L290,160L291,161L293,163L294,165L295,165L297,166L296,167L298,167L300,168L300,169L298,170L296,170L294,169L290,170L292,168L291,167L290,167L289,166L289,164L287,164L285,163L284,162L281,161L280,161L281,160L279,160L276,161L275,162L273,163L272,162L274,161L275,160L276,159L278,159L280,158Z`,CY:`M583,116L584,116L585,116L585,117L586,116L586,117L584,118L582,118L582,116L583,116Z`,CZ:`M534,66L535,67L537,67L537,68L538,69L539,68L540,68L540,69L542,69L543,71L542,71L542,72L541,72L541,73L540,73L539,74L538,73L537,73L535,72L534,72L533,74L531,73L530,71L529,71L528,70L528,69L530,68L530,67L532,67L532,66L533,67L534,66Z`,DE:`M531,59L532,60L531,61L532,62L533,63L533,64L534,66L533,67L532,66L532,67L530,67L530,68L528,69L528,70L529,71L530,71L531,73L531,74L530,74L530,76L530,77L529,76L528,76L527,77L525,77L524,77L523,77L522,77L520,76L519,76L517,76L518,74L519,72L515,72L514,71L514,70L514,69L514,67L514,64L515,64L515,63L516,60L515,60L516,59L518,59L519,58L519,57L519,55L520,56L522,55L522,56L524,57L524,58L526,58L528,57L530,58L531,59Z`,DJ:`M616,197L617,197L618,196L619,197L619,199L617,200L619,201L618,203L617,202L616,202L615,202L615,201L615,200L615,198L616,197Z`,DK:`M522,55L520,56L519,55L518,54L518,51L518,50L520,50L521,49L523,48L523,49L522,50L522,51L524,51L523,52L521,54L522,55ZM527,52L528,54L527,56L524,54L524,53L527,52Z`,DO:`M306,176L306,175L305,174L306,174L306,172L306,170L307,170L309,170L310,171L311,171L311,172L313,172L313,173L314,173L315,174L314,176L313,175L311,175L310,175L310,176L309,176L309,175L308,176L306,178L305,177L306,176Z`,DZ:`M477,143L477,142L477,138L482,135L484,135L486,134L487,132L490,131L491,128L492,128L493,127L497,126L497,125L496,124L496,120L495,118L495,116L497,114L500,114L501,112L504,111L508,111L512,111L513,111L516,110L518,110L519,110L521,110L521,112L521,115L521,118L519,120L519,122L522,124L522,125L523,127L525,133L526,136L526,137L525,140L526,142L525,144L526,146L525,147L526,150L526,152L527,154L529,153L531,155L532,157L523,164L515,171L512,172L509,173L508,170L507,170L506,169L505,167L496,159L487,151L477,143Z`,EC:`M291,244L291,247L290,249L287,253L284,255L282,258L282,260L280,262L279,260L278,260L277,260L277,259L278,258L277,256L279,253L278,252L277,253L275,252L276,251L275,247L276,247L277,244L278,242L278,240L279,240L281,238L284,240L285,242L287,242L288,242L289,243L291,244Z`,EE:`M559,44L558,46L559,48L559,49L557,49L555,48L554,47L552,48L552,46L551,47L550,46L549,44L552,44L554,43L557,44L559,44Z`,EG:`M599,162L588,162L578,162L567,162L566,149L565,136L564,134L565,132L564,130L565,128L568,128L571,129L574,130L575,131L577,130L578,129L580,128L582,129L583,131L583,129L585,130L587,130L589,130L591,135L591,136L590,137L590,140L590,141L589,142L588,141L587,139L584,134L584,135L586,138L588,142L590,147L591,149L592,151L595,155L595,156L595,158L598,161L599,162Z`,EH:`M477,142L477,143L477,148L468,148L468,157L466,157L465,159L465,165L455,164L454,166L454,164L460,164L461,163L462,161L463,156L467,152L468,148L469,147L470,145L472,144L473,145L474,145L475,144L477,144L477,142Z`,ER:`M600,190L599,188L600,183L600,180L601,179L603,179L604,177L606,181L607,184L609,186L613,189L614,192L616,194L617,195L618,196L617,197L616,197L615,195L614,193L613,192L612,191L609,189L608,189L607,189L605,189L604,188L603,190L600,190Z`,ES:`M481,110L481,109L482,107L482,106L482,103L481,101L483,101L483,100L483,99L483,97L484,96L484,94L482,94L482,95L480,95L480,93L479,93L478,94L478,92L477,91L481,88L484,89L487,89L490,89L492,89L495,89L496,91L501,92L502,91L504,93L507,92L507,94L505,96L502,97L502,98L500,100L499,102L500,104L499,106L498,108L496,109L495,111L491,111L489,111L487,112L486,114L485,113L484,112L484,110L481,110Z`,ET:`M632,213L625,225L621,225L619,227L617,228L616,229L614,229L613,227L611,229L610,231L608,230L607,230L606,230L602,227L600,227L599,225L599,223L598,223L596,219L595,218L594,216L593,215L591,214L592,212L593,212L594,211L594,208L594,204L596,203L596,201L597,198L598,196L599,193L600,190L603,190L604,188L605,189L607,189L608,189L609,189L612,191L613,192L614,193L615,195L616,197L615,198L615,200L615,201L615,202L616,202L617,202L618,203L617,204L618,206L619,208L621,209L630,213L632,213Z`,FI:`M554,22L555,23L558,25L557,26L560,29L559,31L561,32L561,34L564,35L564,37L562,38L558,41L555,41L551,42L548,43L546,41L544,41L544,38L543,36L544,35L545,33L549,31L550,30L550,29L547,28L546,28L545,24L542,23L539,22L540,21L543,23L545,22L547,23L549,22L549,21L552,20L555,21L554,22Z`,FJ:`M991,303L990,305L988,306L986,307L986,305L988,304L989,304L991,303ZM984,308L985,308L985,309L984,311L982,311L981,311L981,309L983,308L984,308ZM10,303L10,305L9,303L10,303Z`,FK:`M362,423L364,421L366,422L367,420L369,422L369,423L366,424L365,423L364,424L362,423Z`,FR:`M357,228L355,231L354,234L353,235L352,236L351,235L351,234L350,235L349,235L349,233L350,231L350,230L349,228L349,225L350,222L351,222L353,223L356,226L357,228ZM514,71L515,72L519,72L518,74L517,76L517,77L516,77L514,79L514,81L515,80L516,81L516,82L517,83L516,84L517,87L518,87L518,89L516,90L511,89L507,90L507,92L504,93L502,91L501,92L496,91L495,89L497,87L497,81L495,78L493,77L490,75L489,73L492,73L496,73L496,70L498,71L503,69L504,67L506,66L506,67L507,67L508,68L510,70L511,69L513,71L514,71ZM521,92L523,91L523,93L523,96L521,95L521,93L521,92Z`,GA:`M531,235L533,235L534,235L536,235L536,237L537,238L539,238L540,239L538,243L540,245L540,248L540,251L539,253L536,252L535,251L535,252L533,253L532,254L533,256L531,258L528,254L526,251L524,248L525,246L525,245L526,242L526,240L527,239L531,239L531,235Z`,GB:`M486,58L485,58L483,58L484,56L483,55L485,55L488,57L486,58ZM493,60L493,58L492,56L489,56L489,55L490,54L489,53L488,54L488,52L487,51L488,48L489,46L491,46L494,46L491,49L493,48L496,48L495,50L493,53L495,53L496,53L498,56L499,57L500,60L501,61L504,62L504,63L502,64L503,66L501,67L498,67L494,68L493,67L492,69L490,68L488,70L487,69L490,66L492,65L489,65L488,64L491,63L489,61L490,59L493,60Z`,GE:`M596,89L599,89L602,90L606,92L608,92L610,92L611,94L613,94L612,95L614,96L614,97L612,97L610,96L607,97L604,95L601,95L601,94L600,92L599,91L597,90L596,89Z`,GH:`M500,202L500,203L501,205L501,208L501,211L502,212L501,216L502,218L502,220L503,221L499,223L497,225L495,226L492,225L492,223L491,220L492,216L493,213L492,207L492,205L492,202L497,202L498,203L499,202L500,202Z`,GL:`M421,3L427,3L433,3L435,2L441,2L455,2L465,3L462,3L455,3L446,3L447,3L453,3L458,4L461,3L463,4L461,4L465,4L473,4L478,4L479,4L472,5L471,5L466,5L470,5L467,6L466,7L466,8L468,9L465,9L462,9L465,10L465,11L463,12L465,13L461,13L463,14L462,14L460,14L457,14L459,16L455,16L454,16L457,17L459,18L459,19L456,19L455,19L453,18L453,19L450,20L455,20L458,20L452,22L447,23L441,24L439,24L437,25L433,27L428,28L427,29L424,29L421,30L419,31L418,32L417,34L413,36L413,38L411,40L409,42L406,42L404,40L400,40L398,39L398,37L396,34L396,32L396,30L395,28L396,27L395,26L398,23L402,23L403,22L404,20L402,21L400,21L398,22L396,21L397,20L398,19L400,19L404,19L401,18L400,17L398,18L397,17L400,16L399,15L398,14L397,12L395,12L396,11L391,10L388,10L383,10L379,10L377,10L375,9L379,9L383,8L376,8L372,8L373,7L380,7L387,6L388,6L383,5L385,5L392,4L394,4L398,3L403,3L409,3L410,4L415,3L419,3L421,3L425,4L421,3Z`,GM:`M454,193L457,193L458,192L459,192L460,193L461,193L462,192L462,193L461,194L460,194L458,193L457,194L456,194L454,194L454,193Z`,GN:`M462,196L464,196L466,197L468,197L469,198L470,198L471,199L472,199L473,198L474,198L474,197L475,197L476,198L476,199L477,201L476,202L476,203L477,203L477,204L478,205L477,206L477,207L478,208L478,211L477,212L477,213L477,215L476,215L475,216L474,215L474,214L473,211L472,212L471,212L471,211L471,210L471,209L470,207L469,206L467,206L466,207L465,207L465,208L463,210L462,208L461,206L460,206L460,205L460,204L459,203L458,202L460,200L461,200L462,200L462,199L462,198L462,196Z`,GQ:`M527,235L531,235L531,239L527,239L526,240L526,239L527,235Z`,GR:`M567,116L566,117L563,117L563,116L560,116L560,114L561,116L563,115L565,116L567,116ZM556,96L558,96L560,95L562,96L564,96L564,94L565,95L564,97L564,98L562,98L561,97L558,98L560,100L559,100L557,100L556,99L556,101L558,103L557,104L558,105L560,106L560,108L558,107L559,109L557,109L558,112L557,112L554,111L553,108L553,106L551,104L550,102L550,101L551,100L551,99L552,98L551,98L553,97L554,97L555,97L556,96Z`,GT:`M248,189L248,188L248,187L250,184L253,184L254,182L253,182L253,181L252,181L251,179L253,179L253,177L256,177L258,177L258,180L257,184L258,185L259,184L259,185L258,186L256,187L256,188L256,189L256,190L255,190L254,191L253,192L252,192L250,192L249,191L248,189Z`,GW:`M454,197L456,197L457,197L457,196L462,196L462,198L462,199L462,200L461,200L460,200L458,202L457,201L456,200L455,199L454,198L454,197Z`,GY:`M343,236L342,236L341,236L340,237L339,238L338,238L337,239L336,238L334,237L334,235L333,233L334,230L335,229L334,227L333,226L334,225L333,224L332,224L330,221L331,220L331,218L332,218L333,217L332,216L333,214L335,212L337,213L338,216L338,218L339,218L341,220L342,221L341,224L339,225L340,226L339,228L340,231L341,231L341,233L343,236Z`,HN:`M273,188L272,188L271,188L270,188L269,189L268,188L267,189L267,190L266,191L265,191L265,192L264,191L264,192L263,192L262,192L262,194L261,195L260,195L260,194L259,194L260,192L259,192L259,191L258,192L258,191L257,191L256,190L256,189L256,188L256,187L258,186L259,185L260,185L260,184L261,184L261,185L262,184L263,185L264,185L265,184L266,184L267,184L268,184L269,184L270,184L271,185L271,186L272,187L273,188Z`,HR:`M539,80L540,80L542,82L544,82L545,82L545,83L546,84L545,85L544,84L543,84L540,84L539,84L538,84L538,85L539,86L539,87L541,89L542,89L543,91L545,92L542,91L541,90L539,89L536,87L537,87L536,85L535,84L534,84L533,85L532,84L532,83L533,83L534,83L535,82L535,83L536,83L536,82L537,82L537,81L539,80Z`,HT:`M306,170L306,172L306,174L305,174L306,175L306,176L304,176L302,176L301,176L299,176L298,175L299,174L301,175L303,175L304,174L303,173L303,171L302,171L303,170L304,170L306,170Z`,HU:`M551,74L553,75L553,76L552,76L551,78L550,80L548,81L546,81L545,82L544,82L542,82L540,80L539,80L538,79L539,77L538,76L539,76L539,75L541,76L542,76L544,76L544,75L545,75L546,75L547,74L548,73L551,74Z`,ID:`M891,253L891,265L889,277L887,274L885,274L884,275L880,275L882,272L884,271L883,267L882,264L877,260L875,260L871,257L870,258L869,259L868,257L868,256L866,254L869,253L871,253L871,252L867,252L866,249L864,249L863,247L866,246L868,245L872,246L873,248L873,254L876,256L878,252L882,250L884,250L887,251L889,252L891,253ZM845,277L845,278L843,281L841,282L840,282L841,280L842,278L845,277ZM872,269L872,266L872,265L873,264L873,265L873,267L872,269ZM827,228L826,231L828,235L827,237L831,240L827,240L826,243L826,246L824,249L824,253L822,258L822,257L819,259L818,256L816,256L814,255L811,256L810,255L808,255L806,254L806,249L804,248L803,245L803,242L803,238L805,236L805,238L807,240L809,240L811,240L812,238L813,238L816,239L818,238L820,233L821,232L821,227L825,227L827,228ZM859,254L862,255L863,258L861,256L858,256L857,256L855,256L856,254L859,254ZM852,258L850,257L850,255L853,255L853,256L852,258ZM855,235L855,237L857,238L857,239L857,242L856,242L855,244L857,246L856,247L855,244L854,240L854,237L855,235ZM841,240L845,240L847,237L848,238L846,242L844,242L841,242L836,242L834,242L833,245L836,249L837,247L843,246L842,247L841,247L840,249L837,250L840,255L839,257L842,261L841,263L840,264L839,263L840,260L838,262L837,260L837,259L836,257L836,253L834,254L834,259L834,264L832,265L831,263L832,260L832,256L830,256L830,254L831,251L831,248L833,243L833,241L836,238L838,240L841,240ZM832,282L828,279L831,278L832,279L833,281L833,282L832,282ZM835,275L837,275L840,274L839,276L835,277L831,276L831,275L834,274L835,275ZM827,275L829,274L829,276L826,277L824,277L822,277L824,275L825,275L826,274L827,275ZM800,267L801,269L806,269L807,268L812,269L813,272L817,272L820,275L816,276L814,275L811,275L808,274L806,274L803,272L801,272L799,272L795,271L794,269L792,269L794,265L797,266L799,267L800,267ZM790,247L790,250L791,252L793,252L795,255L794,259L793,265L790,265L788,262L785,259L783,257L782,254L780,251L778,246L776,243L775,239L774,237L771,234L770,231L768,229L764,225L764,223L766,223L770,224L773,227L775,230L777,231L779,236L782,236L785,238L786,241L788,243L787,246L789,247L790,247Z`,IE:`M486,58L487,60L485,63L481,65L477,64L479,61L479,58L482,56L483,55L484,56L483,58L485,58L486,58Z`,IL:`M592,125L591,126L590,125L590,127L591,128L590,128L590,129L591,129L592,130L591,136L591,135L589,130L589,128L589,127L590,124L590,123L591,123L592,123L592,124L592,125Z`,IN:`M755,140L755,141L755,142L756,144L754,143L751,146L751,148L751,151L751,153L751,155L748,155L749,158L749,160L750,161L748,162L746,156L745,156L745,159L743,157L743,155L745,154L745,151L743,151L741,151L738,150L737,148L736,148L734,146L733,149L736,151L734,152L734,153L736,154L736,156L738,159L739,162L739,163L737,163L734,164L734,167L733,169L730,171L728,175L726,178L724,180L724,182L723,183L720,184L719,184L719,187L720,192L720,195L719,198L720,205L719,205L718,208L719,209L716,210L715,213L714,214L712,210L710,205L708,201L707,199L706,196L704,191L704,189L700,184L698,177L697,172L696,168L695,164L692,167L690,166L685,162L686,160L685,159L682,156L683,154L689,154L688,151L686,149L685,146L683,145L685,141L688,141L690,137L691,134L692,130L692,128L693,126L691,124L689,122L688,119L688,118L693,118L695,118L697,115L701,119L701,122L703,124L703,125L701,125L703,129L706,131L711,133L709,135L709,138L712,139L715,141L719,143L723,143L725,145L727,146L730,146L732,146L732,145L731,143L731,141L732,140L733,143L734,144L736,145L738,145L740,145L742,145L742,143L741,142L743,141L744,139L747,136L749,137L750,136L752,138L752,139L755,140Z`,IQ:`M601,126L599,122L604,119L605,115L604,112L605,111L606,109L607,109L610,109L611,110L612,110L615,113L616,114L617,116L616,118L616,120L618,123L622,125L623,128L623,130L624,130L624,132L626,134L625,134L623,134L621,137L617,137L608,130L604,127L601,126Z`,IR:`M626,134L624,132L624,130L623,130L623,128L622,125L618,123L616,120L616,118L617,116L616,114L615,113L612,110L610,107L611,106L609,102L610,101L611,102L613,104L614,104L615,104L618,102L619,102L620,103L619,104L621,106L622,106L623,108L625,109L628,111L631,111L635,110L635,109L637,109L638,107L640,107L641,106L643,107L646,108L648,109L652,112L654,112L655,115L655,119L654,121L656,122L655,124L657,126L657,128L659,129L660,131L658,134L660,136L661,138L664,140L665,143L666,144L667,145L663,147L663,151L658,150L655,149L652,149L650,144L649,144L647,144L644,146L641,145L638,142L635,141L633,138L630,133L629,134L627,133L626,134Z`,IS:`M472,27L471,29L473,30L470,32L464,34L462,34L460,34L454,33L456,32L452,31L456,30L452,29L454,28L457,28L459,29L463,28L465,28L468,27L472,27Z`,IT:`M525,79L526,79L528,78L529,79L533,80L532,81L533,83L531,82L529,83L530,85L529,86L530,87L533,89L534,91L537,94L539,94L539,95L541,96L543,97L545,99L545,100L545,101L544,99L541,99L541,101L542,102L542,104L541,104L540,107L539,107L539,106L539,104L540,104L539,102L538,100L537,100L536,98L534,98L533,96L531,96L529,95L527,93L525,91L524,88L523,87L521,86L520,87L519,88L518,89L518,87L517,87L516,84L517,83L516,82L516,81L517,82L518,82L520,81L521,81L522,80L523,80L524,80L525,79ZM537,106L539,106L538,109L538,110L538,111L536,110L535,110L531,108L531,106L534,107L537,106ZM521,97L523,96L524,99L524,103L523,103L522,104L521,103L521,99L520,97L521,97Z`,JM:`M290,175L292,175L293,176L293,177L291,177L290,178L289,177L288,176L288,175L289,175L290,175Z`,JO:`M591,126L592,125L595,126L599,122L601,126L600,127L595,129L599,132L598,133L597,134L596,134L595,136L594,137L591,136L592,130L591,129L592,128L591,126Z`,JP:`M851,103L851,106L853,110L854,112L855,114L855,116L853,118L849,118L847,122L845,121L843,118L839,119L837,120L834,121L838,123L839,129L838,130L836,129L835,126L833,125L831,123L833,121L833,119L835,118L835,115L840,114L843,115L842,109L845,111L846,107L847,106L846,102L843,98L843,96L845,96L849,100L851,103ZM847,88L847,86L851,90L848,91L848,94L843,92L844,95L841,95L839,92L838,90L840,89L838,85L837,83L842,86L845,87L847,88ZM838,122L839,120L840,120L841,119L843,120L844,121L843,123L842,122L841,123L841,125L839,124L838,122Z`,KE:`M609,261L605,257L605,255L595,247L594,247L594,243L595,241L596,239L597,236L596,232L596,230L594,227L596,225L598,223L599,223L599,225L600,227L602,227L606,230L607,230L608,230L610,231L611,229L613,227L614,229L616,229L614,233L614,247L615,250L614,251L613,253L612,253L611,256L610,257L610,260L609,261Z`,KG:`M672,93L672,92L674,91L678,92L678,90L679,90L683,91L688,91L691,91L693,92L695,93L691,95L691,96L688,97L688,99L685,98L684,99L682,101L682,102L678,103L674,102L672,102L671,100L674,100L675,99L677,100L679,97L675,96L674,97L672,95L673,93L672,93Z`,KH:`M782,198L781,193L782,190L785,190L788,190L790,192L791,189L794,190L795,193L795,197L791,200L793,202L790,203L788,204L785,204L784,202L782,198Z`,KP:`M817,93L816,93L816,94L816,95L818,97L817,98L817,99L816,100L815,100L814,101L815,102L815,103L816,103L819,105L819,106L818,106L816,106L816,107L815,107L815,108L813,107L813,108L812,107L811,106L810,105L811,105L810,104L810,102L809,102L808,101L806,100L807,98L808,97L809,94L811,95L813,96L811,94L814,92L814,91L817,93Z`,KR:`M815,108L815,107L816,107L816,106L818,106L819,106L819,105L823,109L825,111L827,115L827,116L825,117L824,119L822,119L820,117L820,114L817,111L818,110L815,108Z`,KW:`M625,134L625,135L625,136L627,139L625,139L624,137L621,137L623,134L625,134Z`,KZ:`M701,72L701,74L699,74L700,77L700,78L695,77L696,83L695,83L690,85L695,90L694,91L695,93L693,92L691,91L688,91L683,91L679,90L678,90L678,92L674,91L672,92L672,93L671,94L669,96L668,98L667,98L666,97L663,97L662,94L660,94L660,91L656,88L652,89L649,89L646,86L644,85L639,83L633,84L637,96L635,96L633,94L631,93L629,93L628,95L627,94L628,92L627,91L624,90L622,87L620,87L620,86L623,86L622,84L624,83L626,84L626,81L625,79L622,79L620,78L618,80L616,80L614,80L614,78L612,76L610,76L608,74L609,72L608,71L609,68L611,70L611,68L615,65L618,64L624,66L627,68L629,66L633,66L636,68L636,67L640,67L640,66L635,64L637,62L636,62L638,61L635,59L636,58L644,57L645,56L650,55L651,54L655,55L657,58L659,57L663,58L663,59L665,59L670,57L669,58L673,60L682,67L683,65L687,67L689,66L691,67L693,69L695,69L696,70L699,70L701,72Z`,LA:`M794,190L791,189L790,192L788,190L789,189L788,185L785,182L784,179L782,176L780,175L779,177L778,177L777,176L774,178L774,175L774,171L772,171L771,169L770,168L770,166L772,164L772,165L774,165L772,161L773,160L776,163L778,167L781,167L783,170L781,171L781,172L785,174L788,178L790,182L793,184L794,187L794,190Z`,LB:`M592,123L591,123L590,123L590,120L591,118L593,118L593,119L592,121L592,123Z`,LK:`M726,215L726,219L725,220L723,221L721,218L720,213L721,207L723,209L725,211L726,215Z`,LR:`M477,215L477,216L477,218L476,219L477,220L478,220L479,222L479,223L479,224L479,227L478,227L475,225L473,222L470,220L468,218L469,217L469,216L470,214L472,212L473,211L474,214L474,215L475,216L476,215L477,215Z`,LS:`M576,349L576,350L575,352L575,353L573,354L573,355L572,355L570,352L572,350L573,349L575,348L576,349Z`,LT:`M558,54L558,55L557,56L556,57L554,58L552,58L551,57L550,57L550,56L549,55L547,55L546,53L548,52L552,52L554,52L555,52L558,54Z`,LU:`M514,69L514,70L514,71L513,71L513,69L514,69Z`,LV:`M559,49L560,49L560,51L561,52L559,53L558,54L555,52L554,52L552,52L548,52L546,53L546,51L546,49L548,48L550,50L552,50L552,48L554,47L555,48L557,49L559,49Z`,LY:`M567,162L567,169L564,169L564,171L553,164L542,157L540,159L538,160L536,158L532,157L531,155L529,153L527,154L526,152L526,150L525,147L526,146L525,144L526,142L525,140L526,137L526,136L525,133L526,132L526,131L526,129L527,128L528,127L529,126L529,123L532,124L534,124L536,125L539,126L541,129L543,130L547,131L550,133L551,132L552,130L551,128L552,126L553,125L555,124L559,125L560,126L561,126L562,127L564,127L565,128L564,130L565,132L564,134L565,136L566,149L567,162Z`,MA:`M495,116L495,118L496,120L496,124L497,125L497,126L493,127L492,128L491,128L490,131L487,132L486,134L484,135L482,135L477,138L477,142L477,144L475,144L474,145L473,145L472,144L470,145L469,147L468,148L467,152L463,156L462,161L461,163L460,164L454,164L454,163L456,162L457,160L456,158L457,156L459,154L460,153L461,151L461,149L462,147L464,146L466,142L467,141L469,140L472,138L473,137L475,134L475,130L476,127L476,125L478,123L480,121L482,120L484,116L485,114L487,114L488,116L491,115L493,116L495,116Z`,MD:`M562,75L562,74L564,74L566,75L567,75L568,76L568,77L569,77L569,79L570,79L570,80L571,80L570,80L569,80L568,80L568,81L568,82L568,83L567,83L566,82L566,80L566,79L564,77L563,76L563,75L562,75Z`,ME:`M549,92L548,92L547,93L547,94L546,93L545,92L545,90L546,89L547,90L548,90L549,91L549,92Z`,MG:`M636,290L637,291L637,294L637,298L638,300L637,302L637,303L636,301L636,302L636,304L635,306L635,307L634,310L633,314L631,319L628,326L627,331L625,335L623,336L620,337L619,337L617,335L616,333L617,330L616,327L616,325L617,322L618,322L618,320L620,318L620,315L620,314L619,311L619,308L621,306L621,304L622,303L624,303L625,302L626,302L628,300L630,298L631,296L631,294L632,295L634,292L634,290L635,288L636,290Z`,MK:`M554,93L556,94L556,96L555,97L554,97L553,97L551,98L550,97L550,95L550,94L552,93L553,93L554,93Z`,ML:`M468,197L469,196L468,194L467,193L467,191L467,189L468,188L468,186L469,186L471,187L472,186L474,187L474,186L485,186L486,183L485,183L484,167L483,152L487,151L496,159L505,167L506,169L507,170L508,170L509,173L512,172L512,181L510,183L510,185L508,186L504,186L503,188L501,188L499,188L499,187L497,188L495,189L494,190L492,192L491,193L490,194L489,193L488,194L488,197L486,200L486,201L485,203L485,205L484,205L483,206L483,204L482,205L482,204L481,206L479,205L478,205L477,204L477,203L476,203L476,202L477,201L476,199L476,198L475,197L474,197L474,198L473,198L472,199L471,199L470,198L469,198L468,197Z`,MM:`M770,168L768,169L767,170L765,170L765,174L764,175L766,178L768,181L770,183L769,186L768,187L769,189L771,192L772,194L772,196L774,199L773,202L772,206L771,204L772,201L771,199L770,194L769,193L767,188L766,184L764,180L763,182L760,185L759,185L757,184L757,179L756,176L753,172L753,170L751,170L749,167L748,164L749,165L748,162L750,161L749,160L749,158L748,155L751,155L751,153L751,151L751,148L751,146L754,143L756,144L755,142L755,141L755,140L756,140L758,142L759,143L760,145L761,148L759,151L760,155L763,155L764,158L766,159L766,162L768,163L770,164L771,163L772,164L770,166L770,168Z`,MN:`M702,71L704,71L707,68L710,67L712,68L715,68L717,69L720,69L723,70L724,68L722,66L723,64L726,65L728,65L731,66L733,68L737,69L739,69L742,68L744,69L748,70L750,72L752,71L755,72L757,71L760,71L762,69L763,69L765,70L768,70L768,72L769,75L770,76L771,76L774,76L775,75L777,76L781,78L781,79L779,79L776,79L775,80L775,82L772,83L770,85L768,84L766,84L766,86L768,87L769,88L768,89L767,91L765,92L761,92L758,93L756,95L754,94L751,94L747,92L744,92L741,92L736,91L733,92L731,90L728,87L726,86L723,85L719,84L716,84L715,82L714,79L711,76L707,75L704,74L702,71Z`,MR:`M454,166L455,164L465,165L465,159L466,157L468,157L468,148L477,148L477,143L487,151L483,152L484,167L485,183L486,183L485,186L474,186L474,187L472,186L471,187L469,186L468,186L468,188L467,189L465,186L463,184L462,183L460,182L459,182L457,183L456,182L455,183L455,181L456,180L456,176L456,173L456,171L456,169L455,167L454,166Z`,MW:`M590,278L593,279L594,280L595,281L595,286L594,289L595,294L596,294L597,295L598,298L598,302L596,303L595,306L594,303L594,301L594,299L594,298L593,297L592,297L591,295L590,294L591,291L591,290L591,287L592,284L592,283L592,279L590,278Z`,MX:`M199,125L202,125L206,124L205,125L208,127L213,129L218,129L221,129L221,128L226,128L226,129L227,130L228,132L228,133L228,135L229,136L231,137L233,135L236,135L237,136L237,138L238,140L239,142L238,145L239,147L240,148L242,148L243,148L241,151L240,154L239,159L238,161L238,162L238,164L238,167L240,170L240,172L240,174L243,175L244,176L247,175L249,175L251,174L253,173L255,172L256,170L257,167L257,166L259,165L262,164L265,164L266,164L267,165L266,166L264,168L263,171L264,171L263,173L262,176L261,175L260,175L259,177L258,177L256,177L253,177L253,179L251,179L252,181L253,181L253,182L254,182L253,184L250,184L248,187L248,188L248,189L245,185L244,184L242,183L240,183L238,185L237,185L235,184L233,183L231,182L229,181L226,180L224,178L223,177L222,177L219,176L219,174L216,172L215,170L215,168L216,167L216,166L217,165L217,164L217,163L217,161L217,159L215,156L213,153L213,151L211,149L211,148L212,146L211,145L210,144L210,141L209,141L208,139L207,137L207,136L207,134L207,131L208,130L206,128L205,129L204,128L203,129L203,131L202,133L203,135L204,137L204,138L204,139L204,142L205,142L205,144L206,145L206,149L206,150L206,152L206,154L207,154L208,156L208,157L208,158L206,159L206,157L205,155L203,153L202,152L203,150L203,148L202,147L201,145L201,146L200,145L199,144L198,142L198,141L199,142L201,140L201,139L200,136L199,135L199,133L199,131L199,128L199,125Z`,MY:`M777,219L778,219L780,220L780,222L782,222L783,220L784,220L785,223L787,225L787,228L787,229L787,231L787,233L788,234L790,237L789,239L788,239L785,236L781,233L781,231L779,229L779,226L778,223L778,221L777,219ZM827,228L825,227L821,227L821,232L820,233L818,238L816,239L813,238L812,238L811,240L809,240L807,240L805,238L805,236L807,237L809,236L809,233L810,233L814,232L816,229L817,226L818,228L819,227L820,227L820,225L820,223L822,220L823,217L824,217L826,219L826,221L828,222L830,223L830,225L828,225L829,227L827,228Z`,MZ:`M595,286L597,286L600,287L601,287L603,286L604,285L606,285L609,284L611,282L611,284L611,287L611,290L611,296L611,298L611,301L609,303L607,305L605,307L602,309L598,313L597,313L595,316L594,317L593,319L595,322L595,324L595,325L595,328L594,330L595,331L594,332L593,333L591,335L587,337L586,338L586,339L587,340L586,342L585,341L584,340L584,338L584,337L585,333L584,331L584,326L587,321L588,319L588,316L588,315L588,313L589,310L589,305L588,304L587,304L586,303L585,302L583,302L583,301L583,298L591,295L592,297L593,297L594,298L594,299L594,301L594,303L595,306L596,303L598,302L598,298L597,295L596,294L595,294L594,289L595,286Z`,NA:`M553,335L552,348L550,349L548,350L547,349L545,349L545,347L544,346L543,348L541,345L540,343L540,339L539,337L538,331L538,327L538,325L537,324L536,321L535,316L534,314L532,310L532,308L533,307L535,306L537,306L538,308L539,308L550,308L551,309L558,310L563,308L565,308L567,308L568,309L567,310L566,310L564,311L563,310L559,311L557,311L556,324L553,324L553,335Z`,NC:`M946,321L947,324L948,325L946,326L945,325L944,323L943,322L942,319L942,318L943,318L945,319L945,320L946,321Z`,NE:`M540,159L541,165L542,166L542,167L543,168L542,169L542,177L542,182L538,185L537,190L538,191L538,194L540,194L540,195L539,196L539,197L538,197L537,193L536,193L534,195L532,194L530,193L529,194L528,194L526,195L525,196L521,194L520,195L519,195L518,193L515,192L512,192L511,193L511,195L510,197L510,200L508,198L507,198L506,199L506,196L503,195L503,194L501,191L501,190L501,188L503,188L504,186L508,186L510,185L510,183L512,181L512,172L515,171L523,164L532,157L536,158L538,160L540,159Z`,NG:`M507,220L508,214L508,212L508,209L509,208L510,206L510,205L510,203L510,201L510,200L510,197L511,195L511,193L512,192L515,192L518,193L519,195L520,195L521,194L525,196L526,195L528,194L529,194L530,193L532,194L534,195L536,193L537,193L538,197L539,197L540,198L540,199L540,200L537,203L537,205L536,207L536,208L535,211L534,212L533,214L533,216L533,217L531,219L529,217L528,217L526,219L524,223L524,226L521,227L520,227L519,227L516,227L515,225L514,222L512,220L510,220L507,220Z`,NI:`M270,203L269,203L268,203L268,202L267,202L266,203L265,202L264,202L263,201L262,199L262,198L261,197L259,195L260,195L261,195L262,194L262,192L263,192L264,192L264,191L265,192L265,191L266,191L267,190L267,189L268,188L269,189L270,188L271,188L272,188L273,188L272,189L272,190L272,191L271,193L271,194L271,195L271,197L270,197L270,199L270,200L269,201L269,202L270,203Z`,NL:`M515,60L516,60L515,63L515,64L514,64L514,67L513,66L511,65L509,66L508,66L509,65L511,61L514,59L515,60Z`,NO:`M526,6L527,5L529,5L531,6L537,7L533,7L532,8L531,8L530,9L528,9L524,8L526,8L523,8L519,7L518,6L523,5L524,6L526,6ZM559,21L556,22L554,22L555,21L552,20L549,21L549,22L547,23L545,22L543,23L540,21L539,22L538,22L538,23L534,23L534,24L532,24L531,26L530,28L527,31L528,32L527,33L525,33L524,35L525,38L526,39L526,42L524,44L523,45L522,44L518,47L515,47L512,46L511,43L510,38L512,36L517,34L521,32L524,29L528,25L531,23L536,21L540,20L543,20L546,18L549,18L552,18L559,19L556,20L559,21ZM547,5L545,6L540,6L535,6L534,6L532,6L530,5L535,5L537,5L539,5L543,5L547,5ZM543,8L539,8L536,8L537,8L536,7L540,7L541,8L543,8Z`,NP:`M731,141L731,143L732,145L732,146L730,146L727,146L725,145L723,143L719,143L715,141L712,139L709,138L709,135L711,133L711,132L714,133L717,136L719,136L720,138L722,139L725,140L728,141L731,141Z`,NZ:`M935,387L933,388L930,391L927,392L927,391L926,391L930,388L931,386L929,385L930,384L933,383L935,380L937,378L938,376L938,375L938,374L938,371L939,368L940,368L940,370L941,371L940,374L939,377L941,375L941,376L940,378L941,380L942,380L945,379L946,379L943,382L940,384L939,384L937,385L936,386L935,387ZM908,398L912,396L914,394L918,392L919,391L921,390L924,388L923,389L923,391L926,389L925,391L924,392L922,394L918,396L916,397L915,399L914,399L910,400L908,402L904,405L900,406L898,407L896,407L896,406L893,406L894,405L897,403L903,400L905,399L908,398Z`,OM:`M648,160L648,158L648,157L648,155L649,154L648,154L648,152L650,152L651,154L653,155L655,156L657,156L658,159L659,160L660,160L660,161L659,163L659,164L658,165L658,168L656,168L656,170L656,173L655,173L653,174L653,176L653,177L651,177L650,178L650,179L649,180L647,180L646,181L645,182L643,179L641,173L648,169L649,162L648,160ZM649,149L648,148L649,146L649,147L649,148L649,149Z`,PA:`M286,211L286,212L286,214L286,215L285,215L284,216L284,215L283,213L284,212L283,212L283,211L282,210L280,210L280,211L278,212L278,213L279,215L278,216L277,216L276,216L276,214L275,215L274,213L273,213L272,212L271,212L271,213L271,212L271,211L271,210L272,210L271,209L271,208L272,208L273,209L273,210L274,210L275,211L276,210L278,209L279,209L280,207L282,208L283,208L285,209L285,210L286,211Z`,PE:`M306,259L304,259L303,260L301,261L298,263L298,265L297,266L298,268L296,269L296,271L295,271L297,275L298,277L298,279L300,279L301,281L303,281L305,279L306,284L307,285L309,284L311,290L311,291L311,294L311,297L311,299L311,300L311,302L312,305L311,309L311,310L309,311L306,309L306,308L300,304L295,300L292,298L291,295L291,294L288,289L285,282L282,275L280,273L279,270L277,268L275,266L276,265L274,261L275,258L277,256L278,258L277,259L277,260L278,260L279,260L280,262L282,260L282,258L284,255L287,253L290,249L291,247L291,244L293,245L294,247L295,248L297,252L299,252L301,251L302,252L303,252L306,254L304,257L305,257L306,259Z`,PG:`M891,253L896,256L901,258L903,260L904,262L905,264L909,266L910,268L907,268L907,271L909,273L911,277L912,277L912,279L914,280L913,280L916,282L915,283L913,283L913,282L910,282L908,281L906,279L905,277L904,273L900,272L898,273L896,274L896,277L894,278L892,278L889,277L891,265L891,253ZM924,257L925,258L925,260L924,261L923,259L923,258L922,256L920,255L918,254L919,253L921,254L921,255L923,255L924,257ZM919,265L918,266L916,267L915,267L912,266L911,265L911,264L914,264L915,264L916,262L916,264L918,264L919,262L921,261L920,259L922,259L923,259L922,262L921,264L920,264L919,265ZM929,263L930,264L931,267L932,268L932,269L931,269L930,268L929,265L928,263L929,262L929,263Z`,PH:`M832,196L830,193L832,193L833,195L833,198L832,196ZM838,206L839,205L839,203L840,203L840,205L842,201L842,205L841,206L840,209L840,210L838,207L838,206ZM849,212L850,214L850,216L850,220L848,216L847,218L848,221L848,222L844,220L843,218L844,216L842,214L841,216L840,215L838,218L837,216L838,213L840,212L841,211L842,213L844,212L845,210L846,210L846,207L848,209L849,211L849,212ZM827,209L824,212L825,210L827,207L828,205L829,201L830,204L828,206L827,209ZM832,176L832,177L833,180L833,183L832,184L832,187L833,190L835,190L836,190L840,192L840,194L841,195L841,197L838,195L837,193L836,194L834,192L832,193L830,192L830,190L831,189L830,188L830,190L828,188L827,186L827,183L828,184L827,178L827,175L829,175L831,176L831,175L832,176ZM836,201L835,199L837,200L839,200L839,202L838,203L836,204L836,203L836,201ZM845,198L846,202L844,201L844,202L845,205L844,206L844,203L843,203L842,200L844,201L843,199L841,197L844,197L845,198Z`,PK:`M697,115L695,118L693,118L688,118L688,119L689,122L691,124L693,126L692,128L692,130L691,134L690,137L688,141L685,141L683,145L685,146L686,149L688,151L689,154L683,154L682,156L680,155L678,153L676,150L671,151L667,151L663,151L663,147L667,145L666,144L665,143L664,140L661,138L660,136L658,134L663,136L666,136L667,136L668,135L669,136L672,134L672,131L673,129L675,129L675,128L677,128L678,128L679,127L678,125L678,123L680,122L678,120L681,120L681,119L681,118L681,116L681,115L680,113L681,112L683,111L686,111L687,110L688,110L691,111L692,114L697,115Z`,PL:`M552,58L552,60L553,61L553,62L552,62L553,64L553,65L555,67L555,68L554,69L552,71L553,72L552,72L550,71L548,71L547,71L546,72L544,71L543,71L542,69L540,69L540,68L539,68L538,69L537,68L537,67L535,67L534,66L533,64L533,63L532,62L531,61L532,60L531,59L533,58L536,57L539,56L541,56L541,57L543,57L546,57L550,57L551,57L552,58Z`,PR:`M320,175L322,175L322,176L321,177L319,177L318,177L318,175L320,175Z`,PS:`M591,129L590,129L590,128L591,128L590,127L590,125L591,126L592,128L591,129Z`,PT:`M478,94L479,93L480,93L480,95L482,95L482,94L484,94L484,96L483,97L483,99L483,100L483,101L481,101L482,103L482,106L482,107L481,109L481,110L480,111L479,110L478,111L478,108L478,106L477,106L476,104L477,102L478,101L478,100L479,98L479,96L478,95L478,94Z`,PY:`M343,318L344,320L345,325L348,326L349,325L351,326L351,327L352,330L352,332L353,332L354,331L355,332L356,334L356,336L356,338L356,341L354,344L352,344L349,344L346,343L347,337L347,336L344,335L340,332L338,331L332,326L333,321L332,319L333,316L338,315L340,315L343,317L343,318Z`,QA:`M635,152L634,150L635,148L635,147L636,149L637,151L637,153L636,153L635,152Z`,RO:`M567,83L568,84L569,83L570,84L571,84L569,85L569,88L567,88L565,87L563,88L561,89L558,88L556,88L555,88L554,87L554,86L554,85L553,86L551,85L551,84L550,83L549,82L548,81L550,80L551,78L552,76L553,76L554,75L555,75L557,75L558,76L559,76L560,75L561,75L562,75L563,75L563,76L564,77L566,79L566,80L566,82L567,83Z`,RS:`M545,82L546,81L548,81L549,82L550,83L551,84L551,85L553,86L554,85L554,86L554,87L554,88L554,89L555,90L555,91L554,92L555,92L554,93L553,93L552,93L552,92L553,92L552,92L552,91L551,91L551,90L550,90L550,91L549,91L548,90L547,90L546,89L547,89L547,87L546,86L546,85L545,85L546,84L545,83L545,82Z`,RU:`M833,18L834,18L836,19L834,19L833,18ZM616,80L615,82L613,82L611,86L614,89L615,91L618,94L617,96L617,97L616,96L614,94L613,94L611,94L610,92L608,92L606,92L602,90L599,89L596,89L593,87L590,86L587,84L589,83L590,81L589,79L592,78L590,78L589,77L590,76L593,76L593,75L592,73L592,71L588,70L587,70L585,68L584,69L581,68L581,67L579,66L578,66L577,65L578,64L576,63L574,63L573,63L572,63L571,62L570,61L570,60L572,60L573,60L572,59L570,59L570,58L569,58L568,56L568,55L567,54L565,53L564,54L564,53L561,52L560,51L560,49L559,49L559,48L558,46L559,44L561,42L558,41L562,38L564,37L564,35L561,34L561,32L559,31L560,29L557,26L558,25L555,23L554,22L556,22L559,21L561,20L564,22L570,22L578,24L580,25L580,27L578,28L575,28L566,27L565,27L569,29L569,30L570,32L573,33L574,33L574,32L573,31L574,30L579,32L581,31L579,29L582,27L584,27L586,28L587,27L585,26L585,24L583,23L589,24L590,25L588,25L589,26L591,27L593,27L593,25L597,24L602,22L604,23L603,24L605,24L606,23L610,23L612,22L615,24L616,22L614,21L620,21L623,22L632,24L632,23L630,22L629,22L627,21L627,20L625,19L624,18L627,17L627,15L628,15L633,15L634,16L633,18L635,18L637,20L638,22L641,23L641,25L640,28L642,28L642,27L644,27L644,26L645,25L643,24L643,22L641,22L639,21L639,19L636,18L638,16L637,15L638,15L639,16L640,18L642,18L640,17L643,16L647,16L651,17L648,16L646,14L649,14L653,14L657,13L655,13L656,12L658,12L661,11L665,11L665,10L670,10L672,10L675,10L678,10L678,9L679,9L682,8L686,8L684,9L688,9L689,10L691,9L696,9L701,10L703,11L702,12L698,13L700,13L703,14L704,13L707,14L710,14L716,14L717,15L725,15L724,14L728,14L731,14L735,15L737,16L737,17L741,18L745,19L745,17L749,18L751,17L756,18L756,17L760,18L756,16L757,15L775,16L779,17L786,19L793,18L797,19L800,19L802,21L805,21L807,21L810,21L815,21L818,21L824,23L825,22L822,21L821,20L828,21L832,21L839,21L843,22L858,31L857,32L854,31L858,33L862,35L864,35L866,36L866,37L862,36L859,38L857,38L857,40L856,41L857,43L851,41L849,43L846,42L846,43L843,43L844,44L845,47L846,48L850,48L854,52L852,52L854,55L856,56L854,57L857,60L854,61L857,64L856,66L853,65L848,60L840,54L837,51L837,49L835,48L838,47L837,44L838,41L839,40L835,36L833,36L835,38L833,41L827,38L823,39L824,43L827,44L823,45L820,45L818,44L815,43L813,44L806,44L800,45L799,50L797,56L801,56L804,58L807,59L807,57L809,58L816,61L818,63L819,66L822,69L825,74L825,78L826,80L825,84L825,88L825,89L823,91L821,91L819,90L817,92L817,93L816,91L817,91L815,87L812,85L813,83L817,84L816,81L814,78L814,77L813,74L810,75L809,76L805,76L802,73L798,71L793,70L790,67L788,66L786,64L782,61L779,60L775,60L772,60L770,60L769,62L771,62L772,64L772,65L772,68L773,69L771,71L768,70L765,70L763,69L762,69L760,71L757,71L755,72L752,71L750,72L748,70L744,69L742,68L739,69L737,69L733,68L731,66L728,65L726,65L723,64L722,66L724,68L723,70L720,69L717,69L715,68L712,68L710,67L707,68L704,71L702,71L701,72L699,70L696,70L695,69L693,69L691,67L689,66L687,67L683,65L682,67L673,60L669,58L670,57L665,59L663,59L663,58L659,57L657,58L655,55L651,54L650,55L645,56L644,57L636,58L635,59L638,61L636,62L637,62L635,64L640,66L640,67L636,67L636,68L633,66L629,66L627,68L624,66L618,64L615,65L611,68L611,70L609,68L608,71L609,72L608,74L610,76L612,76L614,78L614,80L616,80ZM660,4L663,4L667,5L672,6L673,7L669,7L664,6L661,6L658,5L656,5L660,4ZM677,6L682,7L683,7L673,8L675,6L676,6L677,6ZM746,10L751,10L758,11L759,12L752,12L750,12L745,11L744,10L746,10ZM765,11L770,12L769,12L766,12L761,12L761,11L765,11ZM755,14L757,14L761,14L762,15L759,15L755,14ZM577,5L580,5L582,5L583,5L584,5L585,4L588,5L587,5L585,5L584,5L583,5L582,5L580,5L577,5ZM550,57L546,57L543,57L544,56L547,55L549,55L550,56L550,57ZM597,14L600,12L603,11L608,10L614,10L617,9L620,9L622,10L621,10L615,11L610,11L605,13L604,14L602,16L603,17L608,19L607,19L600,19L599,18L596,18L595,17L597,16L596,15L599,14L597,14ZM817,59L821,62L823,64L827,67L834,72L830,71L832,76L837,79L839,81L836,79L836,81L833,79L831,76L828,73L827,71L823,67L819,64L816,60L816,59L814,57L815,57L817,59ZM160,26L158,27L158,28L160,26L165,26L166,28L162,29L159,30L155,32L153,32L152,32L151,31L150,31L151,30L150,29L147,30L147,29L149,28L146,29L145,30L142,31L157,22L159,24L160,26ZM166,19L164,19L166,18L166,17L168,17L170,18L169,18L166,19ZM579,81L580,81L581,81L582,81L583,82L584,83L587,83L586,84L584,85L581,86L580,86L580,84L577,83L579,82L579,81Z`,RW:`M584,248L586,250L585,252L583,252L582,254L581,254L581,252L581,249L582,248L583,249L584,248Z`,SA:`M591,136L594,137L595,136L596,134L597,134L598,133L599,132L595,129L600,127L601,126L604,127L608,130L617,137L621,137L624,137L625,139L627,139L628,142L630,143L630,144L632,145L633,147L632,148L633,149L634,150L634,151L635,152L636,153L637,153L637,154L637,155L639,159L647,160L648,160L649,162L648,169L641,173L633,174L631,176L629,180L628,180L627,179L626,179L623,179L620,179L619,179L618,178L617,180L618,181L617,183L616,181L615,180L615,178L613,177L612,174L611,171L609,169L607,168L605,165L605,162L605,160L603,156L601,155L600,154L599,152L599,151L598,149L597,148L596,146L594,143L592,141L591,141L591,139L591,137L591,136Z`,SB:`M947,282L947,284L945,284L945,281L946,282L947,282ZM946,279L946,280L944,277L944,274L945,274L945,277L946,279ZM944,280L943,280L941,280L940,279L941,278L943,278L943,279L944,280ZM941,273L942,275L940,274L939,272L938,271L939,272L941,273ZM935,270L936,271L935,271L934,270L933,269L933,268L935,270Z`,SD:`M568,213L566,211L565,210L565,209L565,207L565,206L563,203L563,202L563,201L562,200L562,198L561,196L560,196L560,195L561,194L561,192L562,191L561,190L562,188L563,185L565,185L564,171L564,169L567,169L567,162L578,162L588,162L599,162L600,166L599,166L600,170L602,174L603,175L604,177L603,179L601,179L600,180L600,183L599,188L600,190L599,193L598,196L597,198L596,201L596,203L594,204L594,208L594,211L594,208L593,208L593,206L593,205L591,203L591,201L591,198L590,198L590,199L588,199L589,200L589,202L588,204L586,207L585,207L583,205L582,206L581,207L580,208L577,208L575,207L574,208L573,208L572,206L571,205L569,205L568,207L568,210L567,211L566,211L568,213Z`,SE:`M523,45L524,44L526,42L526,39L525,38L524,35L525,33L527,33L528,32L527,31L530,28L531,26L532,24L534,24L534,23L538,23L538,22L539,22L542,23L545,24L546,28L547,28L544,29L542,31L543,32L540,34L536,36L535,39L537,41L539,42L538,45L536,46L535,50L535,52L532,52L531,54L528,54L527,52L525,49L523,45Z`,SI:`M533,80L534,80L536,79L538,79L539,80L537,81L537,82L536,82L536,83L535,83L535,82L534,83L533,83L532,81L533,80Z`,SK:`M552,72L552,73L551,74L548,73L547,74L546,75L545,75L544,75L544,76L542,76L541,76L539,75L539,74L540,73L541,73L541,72L542,72L542,71L543,71L544,71L546,72L547,71L548,71L550,71L552,72Z`,SL:`M463,210L465,208L465,207L466,207L467,206L469,206L470,207L471,209L471,210L471,211L471,212L472,212L470,214L469,216L469,217L468,218L466,216L464,214L464,213L463,210Z`,SN:`M454,193L453,190L452,189L453,188L454,185L455,183L456,182L457,183L459,182L460,182L462,183L463,184L465,186L467,189L467,191L467,193L468,194L469,196L468,197L466,197L464,196L462,196L457,196L457,197L456,197L454,197L454,194L456,194L457,194L458,193L460,194L461,194L462,193L462,192L461,193L460,193L459,192L458,192L457,193L454,193Z`,SO:`M615,250L614,247L614,233L616,229L617,228L619,227L621,225L625,225L632,213L634,210L635,208L635,206L635,202L635,201L636,201L637,200L638,200L639,199L640,199L641,200L640,202L641,204L640,205L640,209L638,213L637,218L635,223L632,228L629,233L627,236L622,239L620,242L617,247L616,249L615,250Z`,SR:`M349,235L347,234L346,234L345,234L344,235L345,236L344,237L343,236L341,233L341,231L340,231L339,228L340,226L339,225L341,224L342,221L345,222L345,221L348,221L350,222L349,225L349,228L350,230L350,231L349,233L349,235Z`,SS:`M586,230L583,228L582,226L581,227L580,227L579,227L578,227L576,224L575,223L573,221L573,219L571,217L570,215L569,214L568,213L566,211L567,211L568,210L568,207L569,205L571,205L572,206L573,208L574,208L575,207L577,208L580,208L581,207L582,206L583,205L585,207L586,207L588,204L589,202L589,200L588,199L590,199L590,198L591,198L591,201L591,203L593,205L593,206L593,208L594,208L594,211L593,212L592,212L591,214L593,215L594,216L595,218L596,219L598,223L596,225L594,227L593,229L591,229L588,230L587,229L586,230Z`,SV:`M256,190L257,191L258,191L258,192L259,191L259,192L260,192L259,194L257,194L256,194L255,193L254,193L253,192L254,191L255,190L256,190Z`,SY:`M592,125L592,124L592,123L592,121L593,119L593,118L591,118L591,115L591,114L592,113L592,111L593,111L596,110L597,111L599,111L602,110L603,110L606,109L605,111L604,112L605,115L604,119L599,122L595,126L592,125Z`,SZ:`M585,341L584,343L582,343L581,342L581,340L582,339L582,338L583,338L584,338L584,340L585,341Z`,TD:`M564,171L565,185L563,185L562,188L561,190L562,191L561,192L561,194L560,195L560,196L561,196L562,198L562,200L563,201L563,202L561,202L560,204L558,208L555,210L553,209L552,210L552,211L551,212L550,214L546,215L546,214L545,214L545,215L542,216L543,215L542,212L541,211L540,210L539,208L539,206L540,206L541,206L543,206L541,203L541,200L541,198L540,195L540,194L538,194L538,191L537,190L538,185L542,182L542,177L542,169L543,168L542,167L542,166L541,165L540,159L542,157L553,164L564,171Z`,TF:`M660,413L661,414L663,415L661,416L658,417L658,415L659,414L660,413Z`,TG:`M502,202L502,204L503,205L504,207L504,209L505,209L504,218L505,220L503,221L502,220L502,218L501,216L502,212L501,211L501,208L501,205L500,203L500,202L502,202Z`,TH:`M788,190L785,190L782,190L781,193L782,198L779,196L777,196L777,193L774,193L775,197L774,203L773,206L774,209L776,209L777,212L778,216L780,218L781,218L783,220L782,222L780,222L780,220L778,219L777,219L776,218L775,216L774,214L772,212L772,214L771,212L771,210L772,206L773,202L774,199L772,196L772,194L771,192L769,189L768,187L769,186L770,183L768,181L766,178L764,175L765,174L765,170L767,170L768,169L770,168L771,169L772,171L774,171L774,175L774,178L777,176L778,177L779,177L780,175L782,176L784,179L785,182L788,185L789,189L788,190Z`,TJ:`M670,110L670,106L669,104L667,103L667,102L669,102L670,100L670,98L673,97L673,99L674,100L675,99L674,100L671,100L672,102L674,102L678,103L682,102L684,105L686,106L686,107L687,109L685,109L683,108L682,110L681,110L680,111L679,110L678,107L678,106L676,105L675,106L675,108L674,108L673,110L672,109L671,110L670,110Z`,TL:`M845,277L846,276L848,275L850,275L851,274L852,275L851,276L848,277L845,278L845,277Z`,TM:`M628,95L629,93L631,93L633,94L635,96L637,96L639,96L639,94L640,93L642,91L646,93L647,96L648,96L650,96L651,97L654,100L657,102L659,104L662,105L666,107L666,109L664,108L664,109L662,110L662,112L661,113L659,114L659,115L657,116L655,115L654,112L652,112L648,109L646,108L643,107L641,106L640,107L638,107L637,109L635,109L634,107L634,104L631,103L631,100L630,100L630,97L632,98L634,97L632,95L630,93L629,94L629,97L628,95Z`,TN:`M525,133L523,127L522,125L522,124L519,122L519,120L521,118L521,115L521,112L521,110L524,109L526,109L526,111L528,110L527,112L527,114L528,114L527,117L526,119L526,121L528,121L528,123L529,123L529,126L528,127L527,128L526,129L526,131L526,132L525,133Z`,TR:`M612,110L611,110L610,109L607,109L606,109L603,110L602,110L599,111L597,111L596,110L593,111L592,111L592,113L591,114L590,112L591,111L589,112L587,111L586,113L582,113L580,111L577,111L576,113L575,113L572,111L569,111L568,108L566,106L566,104L565,102L567,99L571,99L571,96L576,97L579,95L581,94L585,94L590,96L594,97L597,97L599,97L601,95L604,95L607,97L607,98L607,99L609,100L610,101L609,102L611,106L610,107L612,110ZM564,94L566,93L568,94L569,95L571,96L570,97L568,97L567,98L565,100L564,98L564,97L565,95L564,94Z`,TT:`M330,203L332,203L332,206L330,206L329,206L330,205L330,203Z`,TW:`M824,154L824,159L824,162L821,159L820,157L821,153L822,150L823,151L824,154Z`,TZ:`M594,247L595,247L605,255L605,257L609,261L607,265L607,268L609,269L609,270L608,272L609,273L608,275L609,277L610,281L611,282L609,284L606,285L604,285L603,286L601,287L600,287L597,286L595,286L595,281L594,280L593,279L590,278L589,277L587,276L586,275L585,275L584,270L582,268L582,266L582,264L581,260L583,260L584,259L585,257L585,256L585,255L585,254L585,252L586,250L584,248L585,247L589,247L594,247Z`,UA:`M572,63L573,63L574,63L576,63L578,64L577,65L578,66L579,66L581,67L581,68L584,69L585,68L587,70L588,70L592,71L592,73L593,75L593,76L590,76L589,77L590,78L588,78L586,79L584,79L582,81L583,82L582,81L581,81L580,81L579,81L575,80L574,79L572,80L572,81L570,84L569,83L568,84L567,83L568,83L568,82L568,81L568,80L569,80L570,80L571,80L570,80L570,79L569,79L569,77L568,77L568,76L567,75L566,75L564,74L562,74L562,75L561,75L560,75L559,76L558,76L557,75L555,75L554,75L553,76L553,75L551,74L552,73L552,72L553,72L552,71L554,69L555,68L555,67L553,65L554,65L555,64L557,64L559,64L562,65L564,65L565,65L566,65L568,65L569,66L569,64L570,64L572,63Z`,UG:`M594,247L589,247L585,247L584,248L583,249L582,248L582,246L583,244L583,241L584,239L585,237L586,236L587,235L585,235L586,230L587,229L588,230L591,229L593,229L594,227L596,230L596,232L597,236L596,239L595,241L594,243L594,247Z`,US:`M216,72L223,72L230,72L232,72L239,72L246,72L253,72L260,72L268,72L275,72L280,72L281,71L282,71L281,73L282,73L283,74L285,74L287,75L289,75L291,75L292,75L295,74L296,76L298,77L299,78L301,79L301,80L302,80L302,81L303,81L302,82L304,83L303,86L302,89L301,91L299,92L298,94L299,95L303,93L305,93L309,91L310,90L309,89L311,89L313,89L315,89L317,87L320,85L322,84L325,84L330,84L331,84L332,83L334,82L335,79L338,77L339,78L340,77L341,78L339,82L340,84L340,85L337,87L334,88L332,89L330,90L329,91L328,93L328,94L329,95L330,93L330,94L330,95L328,96L327,95L325,96L323,96L322,96L320,97L323,97L324,97L320,98L319,98L318,99L317,101L314,104L314,103L313,102L313,104L314,104L313,105L312,107L310,109L311,107L310,106L311,103L310,104L310,107L308,106L310,107L309,110L309,112L309,115L306,117L303,118L301,120L300,121L298,122L297,123L294,125L292,127L290,129L289,131L289,134L289,137L289,139L289,141L289,145L289,147L288,148L287,151L286,151L285,151L285,149L284,148L283,145L283,143L283,141L284,139L284,137L282,134L279,135L278,135L277,133L276,132L273,133L271,132L269,133L267,133L268,134L267,136L267,137L266,136L265,137L263,137L262,135L259,135L258,135L256,135L253,136L250,138L247,140L245,141L244,143L243,145L243,147L243,148L242,148L240,148L239,147L238,145L239,142L238,140L237,138L237,136L236,135L233,135L231,137L229,136L228,135L228,133L228,132L227,130L226,129L226,128L221,128L221,129L218,129L213,129L208,127L205,125L206,124L202,125L199,125L200,123L199,121L198,121L198,120L197,120L196,119L194,119L194,118L194,116L193,113L194,108L194,107L194,106L193,104L195,101L194,99L196,97L198,94L199,91L202,88L204,86L206,83L209,79L210,76L210,75L211,74L213,75L213,78L214,77L215,75L216,72ZM81,169L82,170L82,171L80,172L79,173L78,173L79,172L79,170L80,169L80,168L81,169ZM80,167L79,167L79,166L78,166L79,166L80,166L80,167ZM79,165L78,165L77,165L79,165ZM76,163L76,165L75,165L75,164L76,163ZM73,161L73,162L72,163L72,162L73,161ZM153,41L154,42L153,43L151,43L151,42L150,42L153,41ZM173,47L174,48L171,50L167,51L167,50L168,49L171,48L173,47ZM233,21L223,28L206,42L208,42L209,42L209,43L209,45L213,44L216,43L216,44L216,45L217,46L216,48L215,51L217,53L215,55L213,56L213,55L212,54L213,52L213,50L214,47L212,47L209,47L207,46L206,43L205,43L202,42L199,42L196,41L195,40L192,41L190,42L189,43L186,43L183,44L180,44L181,43L185,41L189,40L189,39L184,41L181,42L176,44L176,45L171,47L168,48L165,49L163,50L158,51L155,53L152,54L150,54L147,54L144,55L141,56L136,57L137,56L141,55L144,54L148,53L151,53L153,52L158,50L159,49L162,48L165,47L168,45L164,46L164,45L162,46L162,45L160,46L161,45L158,46L157,46L158,44L160,43L160,42L156,43L156,42L156,41L158,40L158,39L160,37L164,36L166,35L168,35L169,35L172,34L174,34L176,33L177,32L180,31L178,31L176,32L174,32L173,32L170,32L168,31L169,30L168,29L172,28L178,27L180,27L178,28L182,28L183,27L182,26L182,25L182,24L180,23L183,22L186,22L190,21L192,21L196,20L198,19L203,19L205,19L209,18L211,18L211,19L213,19L216,19L215,19L217,19L219,19L222,20L226,20L227,20L230,20L232,21L233,21ZM154,33L155,34L156,33L157,34L159,34L158,35L156,35L155,35L155,34L153,34L154,33Z`,UY:`M350,354L352,353L355,356L356,356L359,358L362,360L364,362L363,364L364,366L363,368L361,370L359,369L357,369L355,368L353,368L351,366L351,364L351,363L350,360L350,356L350,354Z`,UZ:`M637,96L633,84L639,83L644,85L646,86L649,89L652,89L656,88L660,91L660,94L662,94L663,97L666,97L667,98L668,98L669,96L671,94L672,93L673,93L672,95L674,97L675,96L679,97L677,100L675,99L674,100L673,99L673,97L670,98L670,100L669,102L667,102L667,103L669,104L670,106L670,110L668,109L666,109L666,107L662,105L659,104L657,102L654,100L651,97L650,96L648,96L647,96L646,93L642,91L640,93L639,94L639,96L637,96Z`,VE:`M332,224L332,225L331,226L328,228L326,228L325,229L323,228L321,228L320,228L321,229L321,232L322,234L324,234L324,235L322,236L322,238L321,238L318,239L318,240L316,241L314,239L313,235L313,234L312,233L313,231L313,230L312,229L312,227L312,224L313,223L313,221L312,220L311,220L309,220L308,221L306,217L304,217L301,217L300,216L300,215L300,213L300,212L299,211L299,209L298,209L299,207L299,204L300,203L301,202L302,200L304,199L304,200L302,201L303,202L303,204L301,207L302,209L303,209L304,207L303,205L303,202L307,201L307,199L308,198L309,201L310,201L312,203L312,204L315,204L318,204L319,205L321,206L323,205L323,204L326,203L330,203L327,204L328,206L330,207L332,208L332,211L334,211L335,212L333,214L332,216L333,217L332,218L331,218L331,220L330,221L332,224Z`,VN:`M788,204L790,203L793,202L791,200L795,197L795,193L794,190L794,187L793,184L790,182L788,178L785,174L781,172L781,171L783,170L781,167L778,167L776,163L773,160L775,160L777,160L779,159L781,157L783,159L785,159L785,161L787,163L790,164L787,167L786,170L786,173L789,177L792,181L795,184L797,187L800,193L800,200L798,202L796,205L794,208L791,211L789,209L790,206L788,204Z`,VU:`M956,302L957,305L956,305L956,303L956,302ZM955,302L955,301L956,298L957,299L956,302L956,301L955,302Z`,XK:`M550,94L550,93L549,93L549,92L549,91L550,91L550,90L551,90L551,91L552,91L552,92L553,92L552,92L552,93L550,94Z`,YE:`M641,173L643,179L645,182L643,182L642,184L642,185L640,187L636,189L633,191L632,191L631,191L630,193L628,193L626,194L625,194L625,195L624,195L624,196L622,196L621,196L619,196L619,194L618,192L618,191L617,188L616,187L617,187L616,185L617,184L617,183L618,181L617,180L618,178L619,179L620,179L623,179L626,179L627,179L628,180L629,180L631,176L633,174L641,173Z`,ZA:`M543,348L544,346L545,347L545,349L547,349L548,350L550,349L552,348L553,335L554,335L555,338L555,341L555,342L557,341L558,340L560,339L560,337L562,336L563,337L564,338L566,338L568,337L568,336L569,334L570,334L571,333L572,330L575,328L579,325L580,325L581,326L582,325L584,326L584,331L585,333L584,337L584,338L583,338L582,338L582,339L581,340L581,342L582,343L584,343L585,341L586,342L586,344L585,347L584,349L582,350L582,351L580,353L579,354L578,357L574,360L572,362L570,364L567,365L566,365L566,366L564,366L563,367L560,366L559,366L558,366L555,367L553,368L551,369L550,369L549,368L548,368L547,367L547,366L547,364L546,362L547,361L547,359L545,355L544,352L543,348Z`,ZM:`M585,275L586,275L587,276L589,277L590,278L592,279L592,283L592,284L591,287L591,290L591,291L590,294L591,295L583,298L583,301L581,302L579,303L579,304L578,305L575,308L573,310L572,310L569,309L568,309L567,308L565,308L563,308L561,306L560,303L560,291L566,291L566,290L566,289L566,287L566,285L566,284L567,284L567,285L568,285L570,286L571,287L573,288L575,287L575,289L577,289L578,291L579,293L581,293L581,289L579,288L578,287L578,284L579,279L579,278L579,275L580,275L584,274L585,275Z`,ZW:`M584,326L582,325L581,326L580,325L579,325L577,323L575,323L575,320L575,319L574,319L571,315L570,313L569,312L569,309L572,310L573,310L575,308L578,305L579,304L579,303L581,302L583,301L583,302L585,302L586,303L587,304L588,304L589,305L589,310L588,313L588,315L588,316L588,319L587,321L584,326Z`},Hn=[[`overview`,`Overview`],[`pages`,`Pages`],[`sources`,`Sources`],[`audience`,`Audience`],[`behaviour`,`Behaviour`]],Un=[[`people`,`Humans`],[`ai`,`AI agents`],[`bots`,`Bots`],[`all`,`All`]],Wn={people:`Humans`,ai:`AI agents`,bots:`Bots`},Gn=[[`today`,`Today`,0],[`7d`,`7 days`,6],[`30d`,`30 days`,29]],Kn={page:`Page`,country:`Country`,channel:`Channel`,utm_source:`Source`,utm_campaign:`Campaign`,browser:`Browser`,os:`System`,device:`Device`},qn={direct:`Direct`,search:`Search`,social:`Social`,ai:`AI assistants`,referral:`Other sites`,campaign:`Campaigns`,"(none)":`Direct`},Jn={desktop:`Desktop`,mobile:`Phone`,tablet:`Tablet`},Yn={phone:`Phone (< 600 px)`,tablet:`Tablet (600–1023 px)`,laptop:`Laptop (1024–1439 px)`,wide:`Wide (≥ 1440 px)`,"(unknown)":`Unknown`},X=(e=0)=>new Date(Date.now()-e*864e5).toISOString().slice(0,10),Z=e=>(e??0).toLocaleString(),Xn=e=>e>=1?zn(Math.round(e)*1e3,{unitDisplay:`narrow`}):`0s`,Zn=e=>`${Math.round(e*100)}%`,Qn=null;try{Qn=new Intl.DisplayNames([navigator.language||`en`],{type:`region`})}catch{}var $n=e=>{if(e===`(unknown)`)return`Unknown`;if(e===`(other)`)return`Other`;try{return Qn?.of(e)??e}catch{return e}};function er(e,t,{points:n=!1,lowerIsBetter:r=!1}={}){if(t==null||!n&&!t)return null;let i=n?(e-t)*100:(e-t)/t*100,a=Math.round(i);if(a===0)return{text:n?`±0 pt`:`±0%`,tone:`flat`};let o=a>0;return{text:`${o?`+`:`−`}${Math.abs(a)}${n?` pt`:`%`}`,tone:o===r?`bad`:`good`}}var tr=class extends T{static properties={tab:{state:!0},group:{state:!0},preset:{state:!0},from:{state:!0},to:{state:!0},host:{state:!0},filters:{state:!0},metric:{state:!0},compare:{state:!0},data:{state:!0},live:{state:!0},query:{state:!0},notice:{state:!0},off:{state:!0},customOpen:{state:!0}};constructor(){super(),this.tab=`overview`,this.group=`people`,this.preset=`7d`,this.from=X(6),this.to=``,this.host=``,this.filters={},this.metric=`pages`,this.compare=!0,this.data=void 0,this.live=null,this.query=null,this.notice=``,this.off=!1,this.customOpen=!1;try{let e=JSON.parse(localStorage.getItem(`rws-traffic`)??`{}`);Hn.some(([t])=>t===e.tab)&&(this.tab=e.tab),typeof e.compare==`boolean`&&(this.compare=e.compare),Un.some(([t])=>t===e.group)&&(this.group=e.group)}catch{}}createRenderRoot(){return this}connectedCallback(){super.connectedCallback(),this.reload(),this.timer=setInterval(()=>{(!this.to||this.to>=X(0))&&this.reload({quiet:!0})},1e4)}disconnectedCallback(){super.disconnectedCallback(),clearInterval(this.timer)}remember(){try{localStorage.setItem(`rws-traffic`,JSON.stringify({tab:this.tab,compare:this.compare,group:this.group}))}catch{}}get filtered(){return Object.keys(this.filters).length>0}range(){let e=new URLSearchParams({from:this.from});return this.to&&e.set(`to`,this.to),this.host&&e.set(`host`,this.host),e}async reload({quiet:e=!1}={}){e||(this.data=void 0);try{let e=this.range(),[t,n]=await Promise.all([Y(`/traffic?${e}`),Y(`/traffic/live?${this.host?`host=${encodeURIComponent(this.host)}`:``}`).catch(()=>null)]);this.data=t,this.live=n,this.off=!1,this.filtered&&await this.runQuery()}catch(e){e instanceof J&&e.status===404?this.off=!0:this.notice=e.message??String(e)}}async runQuery(){let e=this.periodDays(),t=this.range();t.get(`to`)||t.set(`to`,X(0));for(let[e,n]of Object.entries(this.filters))if(e===`page`){let[e,...r]=n.split(`/`);t.set(`path`,`/${r.join(`/`)}`),this.host||t.set(`host`,e)}else t.set(e,n);let n=new URLSearchParams(t),r=Date.parse(t.get(`from`));n.set(`to`,new Date(r-864e5).toISOString().slice(0,10)),n.set(`from`,new Date(r-e*864e5).toISOString().slice(0,10));try{let[e,r]=await Promise.all([Y(`/traffic/query?${t}`),Y(`/traffic/query?${n}`).catch(()=>null)]);this.query={now:e,before:r}}catch(e){this.query=null,this.notice=e.message??String(e)}}periodDays(){let e=this.to||X(0);return Math.round((Date.parse(e)-Date.parse(this.from))/864e5)+1}setPreset(e){let t=Gn.find(([t])=>t===e);this.preset=e,this.customOpen=!1,this.from=X(t[2]),this.to=``,this.reload()}applyCustom(e){e.preventDefault();let t=this.querySelector(`#traffic-from`)?.value,n=this.querySelector(`#traffic-to`)?.value;t&&(n&&n<t?this.notice=`The start date is after the end date.`:(this.preset=`custom`,this.from=t,this.to=n&&n<X(0)?n:``,this.customOpen=!1,this.notice=``,this.reload()))}setHost(e){this.host=e,this.reload()}pick(e,t){this.data?.store?(this.notice=``,this.filters={...this.filters,[e]:t},this.tab=`overview`,this.runQuery()):this.notice=`Filtering by ${Kn[e].toLowerCase()} needs the traffic database: set [traffic] database_url_env.`}unpick(e){let t={...this.filters};delete t[e],this.filters=t,this.filtered?this.runQuery():this.query=null}setTab(e){this.tab=e,this.remember()}setGroup(e){this.group=e,e===`people`?this.metric===`requests`&&(this.metric=`pages`):(this.tab=`overview`,this.metric=`requests`),this.remember()}pageName(e){if(e===`(other)`)return`Other pages`;if(!(this.host||(this.data?.hosts?.length??0)<=1))return e;let t=e.indexOf(`/`);return t>=0?e.slice(t):e}label(e,t){if(t===`(other)`)return`Other`;switch(e){case`page`:return this.pageName(t);case`country`:return $n(t);case`channel`:return qn[t]??t;case`device`:return Jn[t]??t;default:return t}}ranked(e,{dim:t=null,unit:n=``,limit:r=0,value:i=e=>Z(e[1]),flag:a=!1,pages:o=!1}={}){let s=r?e.slice(0,r):e;if(!s.length)return x`<p class="t-none">Nothing in this period.</p>`;let c=Math.max(...s.map(e=>e[1]),1),l=t&&t in Kn&&!(t in this.filters);return x`<ol class="t-rank" aria-label=${n}>
      ${s.map(([e,n,...r])=>{let s=t?this.label(t,e):o?this.pageName(e):e,u=x`<span class="t-bar" style=${`width:${n/c*100}%`}></span>
          <span class="t-name" title=${e}>${a&&e.length===2?x`<span class="t-code">${e}</span>`:w}${s}</span>
          <span class="t-num">${i([e,n,...r])}</span>`;return x`<li>${l&&e!==`(other)`&&e!==`(unknown)`?x`<button class="t-row" title=${`Filter by ${s}`} @click=${()=>this.pick(t,e)}>${u}</button>`:x`<div class="t-row">${u}</div>`}</li>`})}
    </ol>`}card(e,t,n={},r=null){return x`<section class="t-card" aria-label=${e}>
      <header><h3>${e}</h3>${n.hint?x`<span class="t-hint">${n.hint}</span>`:w}</header>
      ${this.ranked(t,{limit:r?6:0,...n})}
      ${r&&t.length>6?x`<button class="t-more" @click=${()=>this.setTab(r)}>All ${t.length>25?`25`:t.length} →</button>`:w}
    </section>`}numbers(){let e=this.data;if(this.query){let{now:e,before:t}=this.query;return{visitors:[e.visitors,t?.visitors],pages:[e.views,t?.views],sessions:[e.sessions,t?.sessions],bounce:[e.sessions?e.bounces/e.sessions:0,t?.sessions?t.bounces/t.sessions:null],duration:[null,null]}}let t=e.sessions??{},n=e.previous_sessions??{},r=(e.previous?.people?.pages??0)>0||(n.sessions??0)>0;return{visitors:[e.totals.people.visitors,r?e.previous.people.visitors:null],pages:[e.totals.people.pages,r?e.previous.people.pages:null],sessions:[t.sessions??0,r?n.sessions??0:null],bounce:[t.sessions?t.bounces/t.sessions:0,n.sessions?n.bounces/n.sessions:null],duration:[t.sessions?t.duration/t.sessions:0,n.sessions?n.duration/n.sessions:null]}}chartable(e){return e===`pages`?!0:e===`duration`?!1:this.query?e===`visitors`:this.data.bucket===`day`}chartSeries(){let e=this.data,t=this.chartable(this.metric)?this.metric:`pages`,n={pages:`Page views`,visitors:`Visitors`,sessions:`Sessions`,bounce:`Bounce rate`},r,i=null;if(this.query){let e=t===`visitors`?`visitors`:`views`;r=this.query.now.series.map(t=>({x:t.t*1e3,y:t[e]}));let n=Date.parse(this.from);if(this.query.before){let t=n-this.periodDays()*864e5;i=this.query.before.series.map(r=>({x:r.t*1e3-t+n,y:r[e]}))}}else if(t===`pages`)r=e.series.map(e=>({x:e.t*1e3,y:e.pages??0})),i=(e.previous_series??[]).map(e=>({x:e.t*1e3,y:e.pages??0}));else{let n=(e,n)=>{let r=n?e[`previous_${t===`bounce`?`sessions`:t}`]:e[t===`bounce`?`sessions`:t];if(t!==`bounce`)return r;let i=n?e.previous_bounces:e.bounces;return r?Math.round(i/r*100):0};r=(e.days??[]).map(e=>({x:e.t*1e3,y:n(e,!1)})),i=(e.days??[]).map(e=>({x:e.t*1e3,y:n(e,!0)}))}let a=[{name:n[t],points:r}];return this.compare&&i&&i.some(e=>e.y>0)&&a.push({name:`Period before`,points:i}),{series:a,unit:t===`bounce`?`%`:n[t].toLowerCase()}}renderBar(){let e=this.data,t=e?.hosts??[];return x`<div class="t-bar-top">
      <div class="t-seg" role="group" aria-label="Period">
        ${Gn.map(([e,t])=>x`<button aria-pressed=${this.preset===e} @click=${()=>this.setPreset(e)}>${t}</button>`)}
        <button aria-pressed=${this.preset===`custom`} aria-expanded=${this.customOpen} @click=${()=>this.customOpen=!this.customOpen}>
          ${this.preset===`custom`?`${this.from} – ${this.to||`today`}`:`Custom`}
        </button>
      </div>
      <div class="t-seg" role="group" aria-label="Who">
        ${Un.map(([e,t])=>x`<button aria-pressed=${this.group===e} @click=${()=>this.setGroup(e)}>${t}</button>`)}
      </div>
      ${t.length>1?x`<label class="t-select"><span class="t-sr">Site</span>
            <select @change=${e=>this.setHost(e.target.value)}>
              <option value="" ?selected=${!this.host}>All sites</option>
              ${t.map(e=>x`<option value=${e} ?selected=${this.host===e}>${e}</option>`)}
            </select></label>`:w}
      <label class="t-check"><input type="checkbox" .checked=${this.compare} @change=${e=>{this.compare=e.target.checked,this.remember()}}> Compare</label>
      <span class="t-spacer"></span>
      ${this.live?x`<span class="t-live" title=${this.live.pages.map(([e,t])=>`${this.pageName(e)}: ${t}`).join(`
`)||`Nobody right now`}>
            <span class="t-dot" data-on=${this.live.visitors>0}></span>${Z(this.live.visitors)} online</span>`:w}
    </div>
    ${this.customOpen?x`<form class="t-custom" @submit=${e=>this.applyCustom(e)}>
          <label>From <input id="traffic-from" type="date" .value=${this.from} max=${X(0)} min=${X(89)}></label>
          <label>To <input id="traffic-to" type="date" .value=${this.to||X(0)} max=${X(0)} min=${X(89)}></label>
          <lit-material-button type="submit" variant="tonal">Apply</lit-material-button>
          <span class="t-hint">${e?.store?`Up to 90 days; any range with the database in the filter results.`:`Counts are kept for the configured retention_days (30 by default).`}</span>
        </form>`:w}
    ${this.filtered||this.host?x`<div class="t-chips" aria-label="Filters">
          ${this.host?x`<button class="t-chip" @click=${()=>this.setHost(``)}>Site: ${this.host} <span aria-hidden="true">✕</span></button>`:w}
          ${Object.entries(this.filters).map(([e,t])=>x`<button class="t-chip" @click=${()=>this.unpick(e)}>${Kn[e]}: ${this.label(e,t)} <span aria-hidden="true">✕</span></button>`)}
          ${this.filtered?x`<button class="t-chip t-clear" @click=${()=>{this.filters={},this.query=null}}>Clear filters</button>`:w}
        </div>`:w}
    ${this.notice?x`<p class="t-notice" role="status">${this.notice} <button class="t-more" @click=${()=>this.notice=``}>Dismiss</button></p>`:w}`}renderKpis(){let e=this.numbers();return x`<div class="t-kpis" role="group" aria-label="Key numbers">
      ${[[`visitors`,`Visitors`,Z(e.visitors[0]),er(e.visitors[0],e.visitors[1]),`Each person counted once a day`],[`pages`,`Page views`,Z(e.pages[0]),er(e.pages[0],e.pages[1]),`Pages people opened`],[`sessions`,`Sessions`,Z(e.sessions[0]),er(e.sessions[0],e.sessions[1]),`Page views 30 minutes or less apart`],[`bounce`,`Bounce rate`,Zn(e.bounce[0]),er(e.bounce[0],e.bounce[1],{points:!0,lowerIsBetter:!0}),`Sessions of one page view`],[`duration`,`Session length`,e.duration[0]===null?`—`:Xn(e.duration[0]),e.duration[0]===null?null:er(e.duration[0],e.duration[1]),`First page view to last, on average`]].map(([e,t,n,r,i])=>{let a=this.chartable(e);return x`<button class="t-kpi" aria-pressed=${a&&this.metric===e} ?disabled=${!a} title=${i}
          @click=${()=>this.metric=e}>
          <span class="t-kpi-label">${t}</span>
          <span class="t-kpi-value">${n}</span>
          <span class="t-kpi-delta" data-tone=${r?.tone??`none`}>${this.compare&&r?r.text:` `}</span>
        </button>`})}
    </div>`}renderChart(){let{series:e,unit:t}=this.chartSeries(),n=!this.query&&this.data.bucket===`hour`&&this.chartable(this.metric)&&this.metric===`pages`;return x`<section class="t-chart" aria-label="Chart">
      <lit-material-line-chart label=${`${e[0].name} per ${n?`hour`:`day`}`} unit=${t} height="240" .series=${e}
        .formatX=${e=>n?new Date(e).toLocaleTimeString([],{hour:`2-digit`,minute:`2-digit`}):new Date(e).toLocaleDateString([],{month:`short`,day:`numeric`})}></lit-material-line-chart>
    </section>`}lists(){let e=this.data;if(this.query){let e=this.query.now;return{pages:e.pages,channels:e.channels,referrers:e.referrers,utm_source:e.utm_source,utm_campaign:e.utm_campaign,utm_medium:[],countries:e.countries,browsers:e.browsers,os:e.os,devices:e.devices,entry:[],exit:[],not_found:[]}}return{pages:e.people_pages,channels:e.channels??[],referrers:e.referrers,utm_source:e.utm_source??[],utm_medium:e.utm_medium??[],utm_campaign:e.utm_campaign??[],countries:e.countries??[],browsers:e.browsers??[],os:e.os??[],devices:e.devices??[],entry:e.entry_pages??[],exit:e.exit_pages??[],not_found:e.not_found}}renderOverview(){let e=this.lists(),t=this.live;return x`
      ${this.renderKpis()}
      ${this.renderChart()}
      <div class="t-cards">
        ${this.card(`Top pages`,e.pages,{dim:`page`,unit:`views`},`pages`)}
        ${this.card(`Sources`,e.channels,{dim:`channel`,unit:`visits`},`sources`)}
        <section class="t-card t-wide" aria-label="Countries">
          <header><h3>Countries</h3>${e.countries.length>6?x`<button class="t-more" @click=${()=>this.setTab(`audience`)}>All ${e.countries.length} →</button>`:w}</header>
          <div class="t-map-row">
            ${this.renderMap(e.countries)}
            ${this.ranked(e.countries,{dim:`country`,unit:`visitors`,flag:!0,limit:6})}
          </div>
        </section>
        ${this.card(`Devices`,e.devices,{dim:`device`,unit:`visitors`},`audience`)}
        ${t&&!this.filtered?x`<section class="t-card" aria-label="Right now">
              <header><h3>Right now</h3><span class="t-hint">${Z(t.visitors)} ${t.visitors===1?`person`:`people`}, last 5 minutes</span></header>
              ${t.series.some(e=>e.views>0)?x`<lit-material-sparkline label="Page views per minute, last 30 minutes" .values=${t.series.map(e=>e.views)}></lit-material-sparkline>
                    ${this.ranked(t.pages,{dim:`page`,limit:5,unit:`views`})}`:x`<p class="t-none">Nobody has opened a page in the last 30 minutes.</p>`}
            </section>`:w}
      </div>`}renderPages(){let e=this.lists();return x`<div class="t-cards">
      ${this.card(`Top pages`,e.pages,{dim:`page`,unit:`views`})}
      ${e.entry.length?this.card(`Entry pages`,e.entry,{dim:`page`,unit:`sessions`,hint:`Where sessions began`}):w}
      ${e.exit.length?this.card(`Exit pages`,e.exit,{dim:`page`,unit:`sessions`,hint:`Where sessions ended`}):w}
      ${e.not_found.length?this.card(`Not found`,e.not_found,{unit:`requests`,hint:`Paths answered 404`,pages:!0}):w}
    </div>`}renderSources(){let e=this.lists();return x`<div class="t-cards">
      ${this.card(`Channels`,e.channels,{dim:`channel`,unit:`visits`,hint:`How sessions arrived`})}
      ${this.card(`Referring sites`,e.referrers,{unit:`visits`})}
      ${e.utm_source.length||e.utm_campaign.length?x`${this.card(`Campaign sources`,e.utm_source,{dim:`utm_source`,unit:`visits`,hint:`utm_source`})}
            ${e.utm_medium.length?this.card(`Campaign media`,e.utm_medium,{unit:`visits`,hint:`utm_medium`}):w}
            ${this.card(`Campaigns`,e.utm_campaign,{dim:`utm_campaign`,unit:`visits`,hint:`utm_campaign`})}`:w}
    </div>`}renderAudience(){let e=this.lists(),t=this.data,n=e=>{let t=e.reduce((e,[,t])=>e+t,0)||1;return e=>`${Z(e[1])} · ${Zn(e[1]/t)}`};return x`<section class="t-card t-wide t-map-big" aria-label="Map">${this.renderMap(e.countries)}</section>
    <div class="t-cards">
      ${this.card(`Countries`,e.countries,{dim:`country`,unit:`visitors`,flag:!0,value:n(e.countries)})}
      ${this.card(`Devices`,e.devices,{dim:`device`,unit:`visitors`,value:n(e.devices)})}
      ${this.card(`Browsers`,e.browsers,{dim:`browser`,unit:`visitors`,value:n(e.browsers)})}
      ${this.card(`Operating systems`,e.os,{dim:`os`,unit:`visitors`,value:n(e.os)})}
    </div>
    <p class="t-foot">${t.geo_source?x`Countries: <a href="https://db-ip.com" target="_blank" rel="noopener">IP Geolocation by DB-IP</a> (${t.geo_source}, CC BY 4.0), looked up on this server. No addresses are kept.`:`No country table is loaded (built without the geo feature).`}</p>`}renderBehaviour(){let e=this.data;if(![`time_on_page`,`scroll`,`screens`,`events`,`goals`].some(t=>(e[t]??[]).length))return x`<lit-material-empty-state headline=${e.script?`No reports yet`:`Turn on the page script`}>
        ${e.script?x`Add <code>&lt;script defer src="/_rws/t.js"&gt;&lt;/script&gt;</code> to your pages; time on page, scroll depth, screens, events and goals appear here.`:x`Set <code>[traffic] script = true</code> and add <code>&lt;script defer src="/_rws/t.js"&gt;&lt;/script&gt;</code> to your pages. It reports to this server only, with no cookies.`}
      </lit-material-empty-state>`;let t=e.sessions?.sessions??0,n=[`25`,`50`,`75`,`100`].map(t=>[`up to ${t}%`,(e.scroll.find(([e])=>e===t)??[t,0])[1]]),r=(e.event_props??[]).map(([e,t])=>[e.replace(`	`,` · `),t]);return x`<div class="t-cards">
      ${this.card(`Time on page`,(e.time_on_page??[]).map(([e,t,n])=>[e,t,n]),{dim:`page`,unit:`seconds`,value:e=>`${Xn(e[1])} · ${Z(e[2])}×`,hint:`Average, views reported`})}
      ${this.card(`Goals`,e.goals??[],{unit:`conversions`,value:e=>t?`${Z(e[1])} · ${(e[1]/t*100).toFixed(1)}%`:Z(e[1]),hint:`Sessions that reached it`})}
      ${this.card(`Events`,e.events??[],{unit:`events`,hint:`rws.track()`})}
      ${r.length?this.card(`Event properties`,r,{unit:`events`}):w}
      ${this.card(`Scroll depth`,n,{unit:`views`})}
      ${this.card(`Screen sizes`,(e.screens??[]).map(([e,t])=>[Yn[e]??e,t]),{unit:`views`})}
    </div>`}groupKpis(e){let t=this.data,n=t.totals[e],r=t.previous?.[e],i=r&&(r.requests>0||r.visitors>0);return x`<div class="t-kpis" role="group" aria-label="Key numbers">
      ${[[`requests`,`Requests`,n.requests,i?r.requests:null,`Everything they fetched`],[`visitors`,e===`ai`?`Agents`:`Clients`,n.visitors,i?r.visitors:null,`Distinct clients, counted once a day`],[`fetches`,`Pages`,n.pages,i?r.pages:null,`HTML pages they fetched`]].map(([e,t,n,r,i])=>{let a=er(n,r);return x`<button class="t-kpi" aria-pressed=${e===`requests`} ?disabled=${e!==`requests`} title=${i}>
          <span class="t-kpi-label">${t}</span><span class="t-kpi-value">${Z(n)}</span>
          <span class="t-kpi-delta" data-tone=${a?`flat`:`none`}>${this.compare&&a?a.text:` `}</span>
        </button>`})}
    </div>`}groupChart(e){let t=this.data,n=t.bucket===`hour`,r=e.map(e=>({name:Wn[e],points:t.series.map(t=>({x:t.t*1e3,y:t[e]}))}));if(e.length===1&&this.compare){let n=(t.previous_series??[]).map(t=>({x:t.t*1e3,y:t[e[0]]}));n.some(e=>e.y>0)&&r.push({name:`Period before`,points:n})}return x`<section class="t-chart${e.length>1?` t-chart-multi`:``}" aria-label="Chart">
      <lit-material-line-chart label=${`Requests per ${n?`hour`:`day`}`} unit="requests" height="240" .series=${r}
        .formatX=${e=>n?new Date(e).toLocaleTimeString([],{hour:`2-digit`,minute:`2-digit`}):new Date(e).toLocaleDateString([],{month:`short`,day:`numeric`})}></lit-material-line-chart>
    </section>`}renderRobots(e){let t=this.data,n=e=>{let t=new Map;for(let[n,r]of e)t.set(n,(t.get(n)??0)+r);return[...t.entries()].sort((e,t)=>t[1]-e[1])};if(e===`ai`){let e=t.ai_agents.map(e=>[`${e.name}${e.operator?` (${e.operator})`:``}`,e.requests,e]),r=n(t.ai_agents.map(e=>[{"ai-training":`Training crawlers`,"ai-search":`AI search`,"ai-assistant":`Assistants (for a person)`,"ai-agent":`Agents`}[e.kind]??e.kind,e.requests]));return x`
        ${this.groupKpis(`ai`)}
        ${this.groupChart([`ai`])}
        <div class="t-cards">
          ${this.card(`AI agents`,e,{unit:`requests`,value:e=>x`${Z(e[1])}${e[2].verified?x` <lit-material-status status="ok">Verified</lit-material-status>`:w}`,hint:`Verified: from the operator's published network`})}
          ${this.card(`Pages they read`,t.ai_pages,{unit:`requests`,pages:!0})}
          ${this.card(`What for`,r,{unit:`requests`})}
        </div>`}let r=e=>{let t=e.kind.replaceAll(`-`,` `);return e.name&&e.name!==t.replace(`browser no assets`,`browser, no assets`)?`${t} · ${e.name}`:t},i=n(t.bots.map(e=>[e.kind.replaceAll(`-`,` `),e.requests]));return x`
      ${this.groupKpis(`bots`)}
      ${this.groupChart([`bots`])}
      <div class="t-cards">
        ${this.card(`By kind`,i,{unit:`requests`,hint:`Search, SEO, monitors, scanners…`})}
        ${this.card(`Bots`,t.bots.map(e=>[r(e),e.requests]),{unit:`requests`})}
        ${t.not_found.length?this.card(`Not found`,t.not_found,{unit:`requests`,hint:`404s, everyone (scanners mostly)`,pages:!0}):w}
      </div>
      <p class="t-foot">People are browsers that load a page's styles or scripts; a browser that never does is counted here as "browser, no assets". Spoofed: claims to be a known crawler from outside its network.</p>`}renderAll(){let e=this.data,t=[`people`,`ai`,`bots`].map(t=>{let n=e.totals[t],r=e.previous?.[t],i=er(n.requests,r&&r.requests?r.requests:null);return x`<button class="t-kpi" @click=${()=>this.setGroup(t)} title=${`Open ${Wn[t]}`}>
        <span class="t-kpi-label">${Wn[t]}</span><span class="t-kpi-value">${Z(n.visitors)}</span>
        <span class="t-kpi-delta">${Z(n.requests)} requests${this.compare&&i?` · ${i.text}`:``}</span>
      </button>`}),n=[`people`,`ai`,`bots`].reduce((t,n)=>t+e.totals[n].requests,0)||1,r=[`people`,`ai`,`bots`].map(t=>[Wn[t],e.totals[t].requests]);return x`
      <div class="t-kpis" role="group" aria-label="Groups">${t}</div>
      ${this.groupChart([`people`,`ai`,`bots`])}
      <div class="t-cards">
        ${this.card(`Share of requests`,r,{unit:`requests`,value:e=>`${Z(e[1])} · ${Zn(e[1]/n)}`})}
        ${this.card(`Top pages (humans)`,e.people_pages,{dim:`page`,unit:`views`,limit:6})}
        ${this.card(`AI agents`,e.ai_agents.map(e=>[e.name,e.requests]),{unit:`requests`,limit:6})}
      </div>`}renderMap(e){let t=new Map(e.filter(([e])=>e.length===2).map(([e,t])=>[e,t])),n=Math.max(...t.values(),1),r=this.data?.store&&!(`country`in this.filters);return x`<figure class="t-map">
      <svg viewBox=${Bn} role="img" aria-label="Visitors by country">
        ${Object.entries(Vn).map(([e,i])=>{let a=t.get(e)??0,o=a?Math.round(18+72*Math.sqrt(a/n)):0;return S`<path d=${i} class=${a?r?`on can`:`on`:``} style=${a?`--level:${o}%`:``}
            @click=${()=>a&&this.pick(`country`,e)}><title>${$n(e)}${a?`: ${Z(a)}`:``}</title></path>`})}
      </svg>
    </figure>`}render(){if(this.off)return x`<h1>Traffic</h1><lit-material-empty-state headline="Traffic counting is off">It's on whenever the admin API is, unless <code>[traffic] enabled = false</code>.</lit-material-empty-state>`;let e=this.data?this.query===null&&this.filtered&&!this.notice?x`<p class="t-none">Loading…</p>`:this.group===`people`?({overview:()=>this.renderOverview(),pages:()=>this.renderPages(),sources:()=>this.renderSources(),audience:()=>this.renderAudience(),behaviour:()=>this.renderBehaviour()}[this.tab]??(()=>this.renderOverview()))():this.group===`all`?this.renderAll():this.renderRobots(this.group):x`<div class="t-loading">${[0,1,2].map(()=>x`<lit-material-skeleton variant="rect" style="height: 96px"></lit-material-skeleton>`)}</div>`;return x`
      <div class="t-head">
        <h1>Traffic</h1>
        ${this.renderBar()}
        ${this.group===`people`?x`<div class="t-tabs" role="tablist" aria-label="Traffic sections">
              ${Hn.map(([e,t])=>x`<button role="tab" aria-selected=${this.tab===e} @click=${()=>this.setTab(e)}>${t}</button>`)}
            </div>`:x`<div class="t-tabs t-tabs-empty"></div>`}
      </div>
      <div role="tabpanel" aria-label=${this.group===`people`?(Hn.find(([e])=>e===this.tab)??Hn[0])[1]:Wn[this.group]??`All`}>${e}</div>`}};customElements.define(`rws-traffic`,tr);var nr=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},rr=class extends T{constructor(){super(...arguments),this.value=``,this.position=`end`,this.tailLength=8,this.truncated=!1,this.ownTitle=null}static{this.styles=o`
    :host {
      display: inline-flex;
      min-width: 0;
      max-width: 100%;
      vertical-align: bottom;
      white-space: pre;
    }
    :host([hidden]) {
      display: none;
    }
    .start {
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .tail {
      flex: none;
    }
  `}connectedCallback(){super.connectedCallback(),typeof ResizeObserver<`u`&&(this.resizeObserver=new ResizeObserver(()=>this.measure()),this.resizeObserver.observe(this))}disconnectedCallback(){super.disconnectedCallback(),this.resizeObserver?.disconnect()}updated(){this.resizeObserver&&this.measure()}measure(){let e=this.renderRoot.querySelector(`.start`),t=!!e&&e.scrollWidth>e.clientWidth;t!==this.truncated&&(this.truncated=t);let n=this.getAttribute(`title`);(n===null||n===this.ownTitle)&&(t&&n!==this.value?(this.setAttribute(`title`,this.value),this.ownTitle=this.value):!t&&n!==null&&(this.removeAttribute(`title`),this.ownTitle=null))}render(){let e=[...this.value],t=this.position===`middle`&&e.length>this.tailLength&&this.tailLength>0,n=t?e.slice(0,-this.tailLength).join(``):this.value,r=t?e.slice(-this.tailLength).join(``):``;return x`<span part="start" class="start">${n}</span>${t?x`<span part="tail" class="tail">${r}</span>`:w}`}};nr([D()],rr.prototype,`value`,void 0),nr([D({reflect:!0})],rr.prototype,`position`,void 0),nr([D({type:Number,attribute:`tail-length`})],rr.prototype,`tailLength`,void 0),nr([D({type:Boolean,reflect:!0})],rr.prototype,`truncated`,void 0),rr=nr([E(`lit-material-truncate`)],rr);var ir=class extends Mt{constructor(e){if(super(e),this.it=w,e.type!==At.CHILD)throw Error(this.constructor.directiveName+`() can only be used in child bindings`)}render(e){if(e===w||e==null)return this._t=void 0,this.it=e;if(e===C)return e;if(typeof e!=`string`)throw Error(this.constructor.directiveName+`() called with a non-string value`);if(e===this.it)return this._t;this.it=e;let t=[e];return t.raw=t,this._t={_$litType$:this.constructor.resultType,strings:t,values:[]}}};ir.directiveName=`unsafeHTML`,ir.resultType=1;var ar=class extends ir{};ar.directiveName=`unsafeSVG`,ar.resultType=2;var or=jt(ar),sr=new Map,cr=new EventTarget,lr=new Set,ur=`icons-changed`,dr=cr;function fr(e){let t=e.replace(/^\s*(<\?xml[^>]*>\s*)?(<!--[\s\S]*?-->\s*)*/,``).trim(),n=/^<svg\b[^>]*>/i.exec(t);if(!n)return``;let r=n[0].replace(/\s(width|height|aria-hidden|focusable)\s*=\s*("[^"]*"|'[^']*')/gi,``).replace(/\s*\/?>$/,``);return/\sfill\s*=/i.test(r)||(r+=` fill="currentColor"`),r+=` aria-hidden="true" focusable="false">`,r+t.slice(n[0].length)}function pr(e,t={}){for(let[n,r]of Object.entries(e)){let e=t.prefix?`${t.prefix}:${n}`:n,i=fr(r);i?(sr.set(e,i),lr.delete(e)):console.warn(`lit-material-icon: "${e}" is not an <svg>; skipped.`)}cr.dispatchEvent(new Event(ur))}function mr(e){return sr.get(e)}function hr(e){lr.has(e)||(lr.add(e),console.warn(`lit-material-icon: no icon registered as "${e}".`))}var gr=o`
  :host {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    width: 24px;
    height: 24px;
    font-size: 24px;
    line-height: 1;
    color: currentColor;
    vertical-align: middle;
    /* MD3 doesn't define standard color roles for "success"/"warning"/"info"
       the way it does for "error" — matching lit-material-alert's own
       (already dark-mode-corrected) values for visual consistency between
       the two, not a runtime dependency between the packages. */
    --lit-material-icon-info-color: light-dark(#0b57d0, #a8c7fa);
    --lit-material-icon-success-color: light-dark(#146c2e, #83da9e);
    --lit-material-icon-warning-color: light-dark(#7a4d00, #ffb945);
  }

  :host([size="small"]) {
    width: 18px;
    height: 18px;
    font-size: 18px;
  }

  :host([size="large"]) {
    width: 36px;
    height: 36px;
    font-size: 36px;
  }

  ::slotted(svg),
  ::slotted(img),
  svg {
    width: 100%;
    height: 100%;
    display: block;
  }

  :host([color="info"]) {
    color: var(--lit-material-icon-info-color);
  }

  :host([color="success"]) {
    color: var(--lit-material-icon-success-color);
  }

  :host([color="warning"]) {
    color: var(--lit-material-icon-warning-color);
  }

  :host([color="error"]) {
    color: var(--md-sys-color-error, #b3261e);
  }
`,_r=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},vr=class extends T{constructor(){super(...arguments),this.size=`medium`,this.color=`inherit`,this.label=``,this.name=``,this.onIconsChanged=()=>{this.name&&this.requestUpdate()}}static{this.styles=gr}connectedCallback(){super.connectedCallback(),dr.addEventListener(ur,this.onIconsChanged)}disconnectedCallback(){super.disconnectedCallback(),dr.removeEventListener(ur,this.onIconsChanged)}willUpdate(){this.label?(this.setAttribute(`role`,`img`),this.setAttribute(`aria-label`,this.label),this.removeAttribute(`aria-hidden`)):(this.removeAttribute(`role`),this.removeAttribute(`aria-label`),this.setAttribute(`aria-hidden`,`true`))}render(){if(!this.name)return x`<slot part="icon"></slot>`;let e=mr(this.name);if(e===void 0){let e=this.name;return queueMicrotask(()=>mr(e)===void 0&&hr(e)),x``}return or(e)}};_r([D({reflect:!0})],vr.prototype,`size`,void 0),_r([D({reflect:!0})],vr.prototype,`color`,void 0),_r([D()],vr.prototype,`label`,void 0),_r([D({reflect:!0})],vr.prototype,`name`,void 0),vr=_r([E(`lit-material-icon`)],vr);function Q(e,t={}){return x`<lit-material-icon
    name=${e}
    label=${t.label??w}
    size=${t.size??w}
    slot=${t.slot??w}
  ></lit-material-icon>`}pr({gauge:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M207.06,72.67A111.24,111.24,0,0,0,128,40h-.4C66.07,40.21,16,91,16,153.13V176a16,16,0,0,0,16,16H224a16,16,0,0,0,16-16V152A111.25,111.25,0,0,0,207.06,72.67ZM224,176H119.71l54.76-75.3a8,8,0,0,0-12.94-9.42L99.92,176H32V153.13c0-3.08.15-6.12.43-9.13H56a8,8,0,0,0,0-16H35.27c10.32-38.86,44-68.24,84.73-71.66V80a8,8,0,0,0,16,0V56.33A96.14,96.14,0,0,1,221,128H200a8,8,0,0,0,0,16h23.67c.21,2.65.33,5.31.33,8Z"/></svg>`,"chart-line":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M232,208a8,8,0,0,1-8,8H32a8,8,0,0,1-8-8V48a8,8,0,0,1,16,0v94.37L90.73,98a8,8,0,0,1,10.07-.38l58.81,44.11L218.73,90a8,8,0,1,1,10.54,12l-64,56a8,8,0,0,1-10.07.38L96.39,114.29,40,163.63V200H224A8,8,0,0,1,232,208Z"/></svg>`,"tree-structure":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M160,112h48a16,16,0,0,0,16-16V48a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16V64H128a24,24,0,0,0-24,24v32H72v-8A16,16,0,0,0,56,96H24A16,16,0,0,0,8,112v32a16,16,0,0,0,16,16H56a16,16,0,0,0,16-16v-8h32v32a24,24,0,0,0,24,24h16v16a16,16,0,0,0,16,16h48a16,16,0,0,0,16-16V160a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16v16H128a8,8,0,0,1-8-8V88a8,8,0,0,1,8-8h16V96A16,16,0,0,0,160,112ZM56,144H24V112H56v32Zm104,16h48v48H160Zm0-112h48V96H160Z"/></svg>`,certificate:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,136a8,8,0,0,1-8,8H72a8,8,0,0,1,0-16h48A8,8,0,0,1,128,136Zm-8-40H72a8,8,0,0,0,0,16h48a8,8,0,0,0,0-16Zm112,65.47V224A8,8,0,0,1,220,231l-24-13.74L172,231A8,8,0,0,1,160,224V200H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216a16,16,0,0,1,16,16V86.53a51.88,51.88,0,0,1,0,74.94ZM160,184V161.47A52,52,0,0,1,216,76V56H40V184Zm56-12a51.88,51.88,0,0,1-40,0v38.22l16-9.16a8,8,0,0,1,7.94,0l16,9.16Zm16-48a36,36,0,1,0-36,36A36,36,0,0,0,232,124Z"/></svg>`,"sliders-horizontal":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M40,88H73a32,32,0,0,0,62,0h81a8,8,0,0,0,0-16H135a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16Zm64-24A16,16,0,1,1,88,80,16,16,0,0,1,104,64ZM216,168H199a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16h97a32,32,0,0,0,62,0h17a8,8,0,0,0,0-16Zm-48,24a16,16,0,1,1,16-16A16,16,0,0,1,168,192Z"/></svg>`,"list-bullets":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M80,64a8,8,0,0,1,8-8H216a8,8,0,0,1,0,16H88A8,8,0,0,1,80,64Zm136,56H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16Zm0,64H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16ZM44,52A12,12,0,1,0,56,64,12,12,0,0,0,44,52Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,116Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,180Z"/></svg>`,"terminal-window":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,128a8,8,0,0,1-3,6.25l-40,32a8,8,0,1,1-10-12.5L107.19,128,75,102.25a8,8,0,1,1,10-12.5l40,32A8,8,0,0,1,128,128Zm48,24H136a8,8,0,0,0,0,16h40a8,8,0,0,0,0-16Zm56-96V200a16,16,0,0,1-16,16H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216A16,16,0,0,1,232,56ZM216,200V56H40V200H216Z"/></svg>`,"sign-out":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M120,216a8,8,0,0,1-8,8H48a8,8,0,0,1-8-8V40a8,8,0,0,1,8-8h64a8,8,0,0,1,0,16H56V208h56A8,8,0,0,1,120,216Zm109.66-93.66-40-40a8,8,0,0,0-11.32,11.32L204.69,120H112a8,8,0,0,0,0,16h92.69l-26.35,26.34a8,8,0,0,0,11.32,11.32l40-40A8,8,0,0,0,229.66,122.34Z"/></svg>`,"check-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M173.66,98.34a8,8,0,0,1,0,11.32l-56,56a8,8,0,0,1-11.32,0l-24-24a8,8,0,0,1,11.32-11.32L112,148.69l50.34-50.35A8,8,0,0,1,173.66,98.34ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,"warning-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24Zm0,192a88,88,0,1,1,88-88A88.1,88.1,0,0,1,128,216Zm-8-80V80a8,8,0,0,1,16,0v56a8,8,0,0,1-16,0Zm20,36a12,12,0,1,1-12-12A12,12,0,0,1,140,172Z"/></svg>`,"x-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M165.66,101.66,139.31,128l26.35,26.34a8,8,0,0,1-11.32,11.32L128,139.31l-26.34,26.35a8,8,0,0,1-11.32-11.32L116.69,128,90.34,101.66a8,8,0,0,1,11.32-11.32L128,116.69l26.34-26.35a8,8,0,0,1,11.32,11.32ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,pause:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M200,32H160a16,16,0,0,0-16,16V208a16,16,0,0,0,16,16h40a16,16,0,0,0,16-16V48A16,16,0,0,0,200,32Zm0,176H160V48h40ZM96,32H56A16,16,0,0,0,40,48V208a16,16,0,0,0,16,16H96a16,16,0,0,0,16-16V48A16,16,0,0,0,96,32Zm0,176H56V48H96Z"/></svg>`,play:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M232.4,114.49,88.32,26.35a16,16,0,0,0-16.2-.3A15.86,15.86,0,0,0,64,39.87V216.13A15.94,15.94,0,0,0,80,232a16.07,16.07,0,0,0,8.36-2.35L232.4,141.51a15.81,15.81,0,0,0,0-27ZM80,215.94V40l143.83,88Z"/></svg>`,"arrow-clockwise":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M240,56v48a8,8,0,0,1-8,8H184a8,8,0,0,1,0-16H211.4L184.81,71.64l-.25-.24a80,80,0,1,0-1.67,114.78,8,8,0,0,1,11,11.63A95.44,95.44,0,0,1,128,224h-1.32A96,96,0,1,1,195.75,60L224,85.8V56a8,8,0,1,1,16,0Z"/></svg>`,trash:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M216,48H176V40a24,24,0,0,0-24-24H104A24,24,0,0,0,80,40v8H40a8,8,0,0,0,0,16h8V208a16,16,0,0,0,16,16H192a16,16,0,0,0,16-16V64h8a8,8,0,0,0,0-16ZM96,40a8,8,0,0,1,8-8h48a8,8,0,0,1,8,8v8H96Zm96,168H64V64H192ZM112,104v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Zm48,0v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Z"/></svg>`,plus:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M224,128a8,8,0,0,1-8,8H136v80a8,8,0,0,1-16,0V136H40a8,8,0,0,1,0-16h80V40a8,8,0,0,1,16,0v80h80A8,8,0,0,1,224,128Z"/></svg>`,"lock-key":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,112a28,28,0,0,0-8,54.83V184a8,8,0,0,0,16,0V166.83A28,28,0,0,0,128,112Zm0,40a12,12,0,1,1,12-12A12,12,0,0,1,128,152Zm80-72H176V56a48,48,0,0,0-96,0V80H48A16,16,0,0,0,32,96V208a16,16,0,0,0,16,16H208a16,16,0,0,0,16-16V96A16,16,0,0,0,208,80ZM96,56a32,32,0,0,1,64,0V80H96ZM208,208H48V96H208V208Z"/></svg>`,"clipboard-text":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M168,152a8,8,0,0,1-8,8H96a8,8,0,0,1,0-16h64A8,8,0,0,1,168,152Zm-8-40H96a8,8,0,0,0,0,16h64a8,8,0,0,0,0-16Zm56-64V216a16,16,0,0,1-16,16H56a16,16,0,0,1-16-16V48A16,16,0,0,1,56,32H92.26a47.92,47.92,0,0,1,71.48,0H200A16,16,0,0,1,216,48ZM96,64h64a32,32,0,0,0-64,0ZM200,48H173.25A47.93,47.93,0,0,1,176,64v8a8,8,0,0,1-8,8H88a8,8,0,0,1-8-8V64a47.93,47.93,0,0,1,2.75-16H56V216H200Z"/></svg>`,"user-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24ZM74.08,197.5a64,64,0,0,1,107.84,0,87.83,87.83,0,0,1-107.84,0ZM96,120a32,32,0,1,1,32,32A32,32,0,0,1,96,120Zm97.76,66.41a79.66,79.66,0,0,0-36.06-28.75,48,48,0,1,0-59.4,0,79.66,79.66,0,0,0-36.06,28.75,88,88,0,1,1,131.52,0Z"/></svg>`});var yr=[{id:`overview`,label:`Overview`,icon:`gauge`},{id:`traffic`,label:`Traffic`,icon:`chart-line`},{id:`upstreams`,label:`Upstreams`,icon:`tree-structure`},{id:`certificates`,label:`Certificates`,icon:`certificate`},{id:`controls`,label:`Controls`,icon:`sliders-horizontal`},{id:`config`,label:`Config`,icon:`list-bullets`},{id:`logs`,label:`Logs`,icon:`terminal-window`},{id:`audit`,label:`Audit`,icon:`clipboard-text`}];function br(){let e=location.hash.replace(/^#\/?/,``);return yr.some(t=>t.id===e)?e:`overview`}var xr=e=>zn(e*1e3,{unitDisplay:`narrow`}),Sr=x`<svg class="brand-mark" viewBox="0 0 32 32" aria-hidden="true"><rect width="32" height="32" rx="7" fill="#D2551E"/><g transform="translate(4 3.5)"><rect x="2" y="5" width="20" height="4" rx="1.5" fill="#16120F"/><rect x="2" y="10.5" width="20" height="4" rx="1.5" fill="#16120F"/><path d="M3.5 16H15L14 17L15 18L14 19L15 20H3.5A1.5 1.5 0 0 1 2 18.5V17.5A1.5 1.5 0 0 1 3.5 16Z" fill="#F2EBE3"/><rect x="16.5" y="16.5" width="2" height="2" fill="#F2EBE3"/><rect x="19.5" y="15" width="1.4" height="1.4" fill="#F2EBE3"/></g></svg>`,$=(e,t)=>x`<lit-material-status status=${e}>${t}</lit-material-status>`,Cr=e=>new Date(e).toLocaleTimeString(),wr=e=>/"status":5\d\d/.test(e)?`error`:/"status":4\d\d/.test(e)?`warning`:null,Tr=(e,t)=>[{key:`url`,label:`Backend`,mono:!0},{key:`weight`,label:`Weight`,numeric:!0},{key:`healthy`,label:`Health`,render:e=>e.healthy?$(`ok`,`Healthy`):$(`error`,`Failing checks`)},{key:`breaker`,label:`Breaker`,render:e=>e.breaker===null?$(`neutral`,`None`):e.breaker===`closed`?$(`ok`,`Closed`):e.breaker===`half_open`?$(`warning`,`Probing`):$(`error`,`Open`)},{key:`rotation`,label:`In rotation`,render:n=>x`<div class="row">
      <lit-material-switch aria-label="${n.url} in rotation" .checked=${!n.drained} ?disabled=${e.busy===`drain:${n.url}`}
        @change=${r=>e.drain(t,n.url,!r.target.checked)}></lit-material-switch>
      ${n.drained?$(`warning`,`Drained`):w}
    </div>`}],Er=[{key:`name`,label:`Name`,mono:!0},{key:`expires`,label:`Expires`},{key:`days_left`,label:`Days left`,numeric:!0,render:e=>e.days_left<7?$(`error`,String(e.days_left)):e.days_left<21?$(`warning`,String(e.days_left)):e.days_left},{key:`source`,label:`Source`,render:e=>e.source===`on_demand`?`On demand`:`Configured`}],Dr=class extends T{static properties={authed:{state:!0},view:{state:!0},loginError:{state:!0},data:{state:!0},error:{state:!0},samples:{state:!0},paused:{state:!0},filter:{state:!0},busy:{state:!0},me:{state:!0},sso:{state:!0}};constructor(){super(),this.authed=!1,this.view=br(),this.data={},this.samples=[],this.paused=!1,this.filter=``,this.busy=``,this.lastTotal=null,this.me=null,this.sso=!1}createRenderRoot(){return this}connectedCallback(){super.connectedCallback(),this.onHash=()=>this.show(br()),addEventListener(`hashchange`,this.onHash),this.check({quiet:!jn()}),Y(`/auth`).then(e=>this.sso=!!e?.oidc).catch(()=>{})}disconnectedCallback(){super.disconnectedCallback(),removeEventListener(`hashchange`,this.onHash),this.stop()}async check({quiet:e=!1}={}){try{let[e,t]=await Promise.all([Y(`/whoami`),Y(`/status`)]);this.me=e,this.data={...this.data,status:t},this.authed=!0,this.loginError=``,this.show(this.view)}catch(t){if(this.authed=!1,e&&t instanceof J&&t.status===401)return;this.loginError=t instanceof J&&t.status===401?`That token isn't an admin token. Check RWS_ADMIN_TOKEN (or your token_env) on the server.`:t instanceof J&&t.status===403?t.message:`The admin API didn't answer: ${t.message}`,t instanceof J&&t.status===401&&Mn(``)}}login(e){e.preventDefault();let t=(this.querySelector(`#token`)?.value??``).trim();t?(Mn(t),this.check()):this.loginError=`Paste the admin token.`}logout(){let e=this.me?.via===`oidc`;Mn(``),this.stop(),this.authed=!1,this.data={},this.me=null,e&&location.assign(`/admin/auth/logout`)}async call(e,t){try{let n=await Y(e,t);return this.error=``,n}catch(e){throw e instanceof J&&e.status===401?(this.logout(),this.loginError=`The server no longer accepts this token. Sign in again.`):this.error=e.message,e}}toast(e){this.querySelector(`#toast`)?.show(e)}show(e){this.stop(),this.view=e,location.hash!==`#/${e}`&&history.replaceState(null,``,`#/${e}`),this.error=``;let t={overview:2e3,upstreams:3e3,certificates:3e4,controls:5e3,audit:1e4}[e];this.load(e),t&&(this.timer=setInterval(()=>this.load(e),t)),e===`logs`&&this.updateComplete.then(()=>this.follow()),queueMicrotask(()=>this.querySelector(`main`)?.focus({preventScroll:!0}))}stop(){clearInterval(this.timer),this.timer=void 0,this.abort?.abort(),this.abort=void 0}async load(e){let t=async(e,t)=>{try{let n=await this.call(t);this.data={...this.data,[e]:n}}catch{}};if(e===`overview`){await Promise.all([t(`status`,`/status`),t(`metrics`,`/metrics`)]);let e=this.data.metrics?.requests_total;if(typeof e==`number`){let t=Date.now();if(this.lastTotal){let n=Math.max(0,(e-this.lastTotal.total)/((t-this.lastTotal.at)/1e3));this.samples=[...this.samples,{x:t,y:Math.round(n*100)/100}].slice(-60)}this.lastTotal={total:e,at:t}}}else e===`upstreams`?await t(`upstreams`,`/upstreams`):e===`certificates`?await t(`certificates`,`/certificates`):e===`controls`?await Promise.all([t(`maintenance`,`/maintenance`),t(`blocklist`,`/blocklist`),t(`flags`,`/flags`),t(`rateLimit`,`/rate-limit`),t(`caches`,`/caches`),this.loadAuthz()]):e===`config`?await t(`config`,`/config`):e===`audit`&&await t(`audit`,`/audit?n=200`)}async follow(){this.abort=new AbortController;let e=this.querySelector(`lit-material-log-viewer`);if(e){e.highlight=wr;try{e.lines=await this.call(`/logs?n=200`),await Nn(t=>e.appendLines(t),this.abort.signal)}catch(e){e.name!==`AbortError`&&!this.error&&(this.error=`The live log stopped: ${e.message}`)}}}async act(e,t,n,r){this.busy=e;try{let e=await this.call(t,n);r&&this.toast(r(e)),await this.load(this.view)}catch{}finally{this.busy=``}}drain(e,t,n){return this.act(`drain:${t}`,`/upstreams/${encodeURIComponent(e)}/drain`,{method:`PUT`,body:{backend:t,drained:n}},()=>n?`${t} drained: it gets no new requests`:`${t} is back in rotation`)}reload(){return this.act(`reload`,`/reload`,{method:`POST`},e=>e.applied?`Configuration reloaded`:`Reload kept the running configuration: ${e.errors.join(`; `)}`)}block(e){e.preventDefault();let t=this.querySelector(`#block-ip`),n=(t?.value??``).trim();n&&this.act(`block`,`/blocklist`,{method:`POST`,body:{ip:n}},e=>`${e.blocked} blocked`).then(()=>{!this.error&&t&&(t.value=``)})}async loadAuthz(){try{let[e,t]=await Promise.all([Y(`/authz`),Y(`/authz/tuples?limit=200`)]);this.data={...this.data,authz:{schema:e,tuples:t.tuples}}}catch(e){e instanceof J&&e.status===404&&(this.data={...this.data,authz:null})}}addTuple(e){e.preventDefault();let t=this.querySelector(`#authz-tuple`),n=(t?.value??``).trim();n&&this.act(`authz-add`,`/authz/tuples`,{method:`POST`,body:{tuple:n}},e=>e.written?`${e.tuple} added`:`${e.tuple} was already there`).then(()=>{!this.error&&t&&(t.value=``)})}async checkAccess(e){e.preventDefault();let t=e=>(this.querySelector(e)?.value??``).trim(),n={subject:t(`#authz-subject`),relation:t(`#authz-relation`),object:t(`#authz-object`)};if(n.subject&&n.relation&&n.object)try{let e=await this.call(`/authz/check`,{method:`POST`,body:n});this.data={...this.data,authzCheck:{...n,...e}}}catch{}}purge(e){let t=(this.querySelector(`#purge-path`)?.value??``).trim(),n=t?t.endsWith(`*`)?`?prefix=${encodeURIComponent(t.slice(0,-1))}`:`?path=${encodeURIComponent(t)}`:``;return this.act(`purge:${e}`,`/caches/${encodeURIComponent(e)}${n}`,{method:`DELETE`},e=>`${e.purged} ${e.purged===1?`entry`:`entries`} purged from ${e.name}`)}render(){if(!this.authed)return this.renderLogin();let e=this.data.status,t=yr.map(e=>({label:e.label,icon:Q(e.icon)}));return x`
      <div class="shell">
        <header class="top">
          <span class="brand">${Sr}<span class="brand-text">rws admin</span></span>
          ${e?e.ready?$(`ok`,`Ready`):$(`error`,`Not ready`):w}
          <span class="spacer"></span>
          ${e?x`<span class="meta">${e.version} · ${e.listeners?.main?.address??``}</span>`:w}
          ${this.me?x`<span class="meta" title="Signed in ${this.me.via===`oidc`?`with single sign-on`:`with a token`}"><span class="icon">${Q(`user-circle`)}</span> ${this.me.user} · ${this.me.role}</span>`:w}
          <lit-material-button variant="text" @click=${()=>this.logout()}><span slot="icon" class="icon">${Q(`sign-out`)}</span>Sign out</lit-material-button>
        </header>
        <lit-material-adaptive-navigation class="nav" label="Sections" .destinations=${t}
          .selected=${yr.findIndex(e=>e.id===this.view)}
          @change=${e=>this.show(yr[e.target.selected].id)}></lit-material-adaptive-navigation>
        <main id="main" tabindex="-1">
          ${this.error?x`<lit-material-alert variant="error"><span slot="title">Something failed</span>${this.error}</lit-material-alert>`:w}
          ${this.renderView()}
        </main>
      </div>
      <lit-material-snackbar id="toast"></lit-material-snackbar>
    `}renderLogin(){return x`
      <div class="login">
        <section class="login-card tile" aria-labelledby="login-title">
          <h1 id="login-title"><span class="icon">${Q(`lock-key`)}</span>Sign in to rws admin</h1>
          ${this.sso?x`<lit-material-button href="/admin/auth/login" style="width: 100%">Sign in with single sign-on</lit-material-button>
            <p class="lede" style="text-align: center">or with a token</p>`:w}
          <p>Paste an admin token: the value of <code>RWS_ADMIN_TOKEN</code> (or the variable your <code>[admin] token_env</code> names), or your own from <code>[admin] users</code>. It stays in this tab only.</p>
          <form @submit=${e=>this.login(e)} novalidate>
            <lit-material-text-field id="token" variant="outlined" label="Admin token" type="password" autocomplete="current-password" required
              ?error=${!!this.loginError} error-text=${this.loginError??``}></lit-material-text-field>
            <lit-material-button type="submit">Sign in</lit-material-button>
          </form>
        </section>
      </div>
    `}renderView(){switch(this.view){case`upstreams`:return this.renderUpstreams();case`certificates`:return this.renderCertificates();case`controls`:return this.renderControls();case`traffic`:return x`<rws-traffic></rws-traffic>`;case`config`:return this.renderConfig();case`logs`:return this.renderLogs();case`audit`:return this.renderAudit();default:return this.renderOverview()}}skeleton(e=3){return x`<div class="grid" aria-busy="true">${Array.from({length:e},()=>x`<div class="tile"><lit-material-skeleton width="40%"></lit-material-skeleton><lit-material-skeleton variant="rounded" height="32px"></lit-material-skeleton></div>`)}</div>`}renderOverview(){let e=this.data.status,t=this.data.metrics;if(!e||!t)return x`<h1>Overview</h1>${this.skeleton(4)}`;let n=t.requests_total?(t.errors_total/t.requests_total*100).toFixed(2):`0.00`,r=this.samples.map(e=>e.y);return x`
      <h1>Overview</h1>
      <p class="lede">Up ${xr(e.uptime_secs)}, process ${e.pid}. Counters refresh every 2 seconds.</p>
      ${(e.disk??[]).filter(e=>e.low).map(e=>x`<lit-material-alert variant="warning" style="margin-bottom: var(--space-200)">
        <span slot="title">Disk almost full</span>
        ${e.dir} has ${Ln(e.free_bytes)} free of ${Ln(e.total_bytes)}: rws has stopped writing the access log and disk cache there until space is freed.</lit-material-alert>`)}
      <div class="grid">
        <lit-material-stat label="Requests served" value=${t.requests_total.toLocaleString()}>
          ${r.length>1?x`<lit-material-sparkline slot="caption" label="Requests per second" .values=${r}></lit-material-sparkline>`:w}
        </lit-material-stat>
        <lit-material-stat label="Errors" value=${t.errors_total.toLocaleString()} caption="${n}% of requests"></lit-material-stat>
        <lit-material-stat label="Open connections" value=${String(t.active_connections)}></lit-material-stat>
        <lit-material-stat label="Queued for a worker" value=${String(t.thread_pool_queued)}></lit-material-stat>
      </div>
      <section class="section" aria-labelledby="rate-title">
        <h2 id="rate-title">Requests per second</h2>
        ${this.samples.length<2?x`<lit-material-empty-state size="compact" headline="Collecting samples">The chart fills in as this page stays open.</lit-material-empty-state>`:x`<lit-material-line-chart label="Requests per second, last ${Math.round((this.samples.at(-1).x-this.samples[0].x)/1e3)} seconds"
              unit="req/s" height="200" .formatX=${Cr} .series=${[{name:`Requests per second`,points:this.samples}]}></lit-material-line-chart>`}
      </section>
      <section class="section" aria-labelledby="listen-title">
        <h2 id="listen-title">Listeners</h2>
        <div class="grid">
          <lit-material-stat class="mono" label="Main" value=${e.listeners.main.address} caption=${e.listeners.main.tls?`HTTPS, HTTP/2, HTTP/3`:`Plain HTTP`}></lit-material-stat>
          <lit-material-stat class="mono" label="HTTP redirect" value=${e.listeners.redirect??`off`}></lit-material-stat>
          <lit-material-stat class="mono" label="Admin" value=${e.listeners.admin}></lit-material-stat>
        </div>
      </section>
      <section class="section" aria-labelledby="build-title">
        <h2 id="build-title">Build</h2>
        <p class="lede">Version ${e.version} with ${e.features.join(`, `)}.</p>
      </section>
    `}renderUpstreams(){let e=this.data.upstreams;return e?x`
      <h1>Upstreams</h1>
      <p class="lede">Drain a backend before deploying it: new requests go elsewhere, requests already on it finish, and its health checks keep running.</p>
      ${e.length===0?x`<lit-material-empty-state size="compact" headline="No upstreams configured">Upstreams come from <code>[[upstream]]</code> sections in <code>rws.config.toml</code>. Add one, then reload from Controls.</lit-material-empty-state>`:e.map(e=>x`
          <section class="section" aria-labelledby="up-${e.name}">
            <h2 id="up-${e.name}">${e.name}</h2>
            <lit-material-table caption="Backends of ${e.name}" caption-hidden .columns=${Tr(this,e.name)} .rows=${e.backends}></lit-material-table>
          </section>`)}
    `:x`<h1>Upstreams</h1>${this.skeleton(2)}`}renderCertificates(){let e=this.data.certificates;if(!e)return x`<h1>Certificates</h1>${this.skeleton(2)}`;let t=e.acme;return x`
      <h1>Certificates</h1>
      <p class="lede">Every certificate this server presents, soonest expiry first.</p>
      ${e.certificates.length===0?x`<lit-material-empty-state size="compact" headline="No TLS certificates">This server speaks plain HTTP. Set <code>tls_cert_file</code> and <code>tls_key_file</code>, or <code>[acme] domains</code>, to serve HTTPS.</lit-material-empty-state>`:x`<lit-material-table caption="Certificates" caption-hidden .columns=${Er} .rows=${e.certificates}></lit-material-table>`}
      <section class="section" aria-labelledby="acme-title">
        <h2 id="acme-title">ACME</h2>
        ${t.running?x`<p class="lede">Renewal runs automatically. ${t.last_check?x`Last check ${new Date(t.last_check.at*1e3).toLocaleString()}: ${t.last_check.outcome}.`:`No check has finished yet.`}</p>
              <lit-material-button variant="outlined" ?disabled=${this.busy===`renew`}
                @click=${()=>this.act(`renew`,`/certificates/renew`,{method:`POST`},()=>`Renewal requested: the new certificate is swapped in when it's issued`)}>
                <span slot="icon" class="icon">${Q(`arrow-clockwise`)}</span>Renew now</lit-material-button>`:x`<lit-material-empty-state size="compact" headline="ACME is off">Set <code>[acme] domains</code> to have rws obtain and renew certificates itself.</lit-material-empty-state>`}
      </section>
    `}renderControls(){let e=this.data;return!e.maintenance||!e.blocklist||!e.flags?x`<h1>Controls</h1>${this.skeleton(3)}`:x`
      <h1>Controls</h1>
      <p class="lede">Changes here apply at once and last until the server restarts.</p>
      <div class="grid">
        <div class="tile">
          <div class="label">Maintenance mode</div>
          <div class="row" style="margin-top: var(--space-75)">
            <lit-material-switch aria-label="Maintenance mode" .checked=${e.maintenance.active} ?disabled=${this.busy===`maintenance`}
              @change=${e=>this.act(`maintenance`,`/maintenance`,{method:`PUT`,body:{active:e.target.checked}},e=>e.active?`Maintenance on: visitors get 503`:`Maintenance off`)}></lit-material-switch>
            ${e.maintenance.active?$(`warning`,`On: 503 for visitors`):$(`ok`,`Off`)}
          </div>
        </div>
        <div class="tile">
          <div class="label">Configuration</div>
          <p class="label" style="margin: var(--space-50) 0 var(--space-100)">Re-read rws.config.toml and the certificates.</p>
          <lit-material-button variant="tonal" ?disabled=${this.busy===`reload`} @click=${()=>this.reload()}><span slot="icon" class="icon">${Q(`arrow-clockwise`)}</span>Reload</lit-material-button>
        </div>
        <lit-material-stat label="Global rate limit" value=${e.rateLimit?.configured?`${e.rateLimit.max_requests} per ${e.rateLimit.window_secs}s`:`Off`}></lit-material-stat>
      </div>

      <section class="section" aria-labelledby="block-title">
        <h2 id="block-title">Blocked addresses</h2>
        <form class="row" @submit=${e=>this.block(e)} novalidate>
          <lit-material-text-field id="block-ip" variant="outlined" label="IP address" supporting-text="IPv4 or IPv6, e.g. 203.0.113.7"></lit-material-text-field>
          <lit-material-button type="submit" ?disabled=${this.busy===`block`}><span slot="icon" class="icon">${Q(`plus`)}</span>Block</lit-material-button>
        </form>
        ${e.blocklist.length===0?x`<p class="lede" style="margin-top: var(--space-200)">Nobody is blocked.</p>`:x`<lit-material-table style="margin-top: var(--space-200)" caption="Blocked addresses" caption-hidden density="compact"
              .columns=${[{key:`ip`,label:`Address`,mono:!0},{key:`action`,label:`Action`,hiddenLabel:!0,render:e=>x`<lit-material-button variant="text" @click=${()=>this.act(`unblock:${e.ip}`,`/blocklist/${encodeURIComponent(e.ip)}`,{method:`DELETE`},()=>`${e.ip} unblocked`)}>Unblock</lit-material-button>`}]} .rows=${e.blocklist.map(e=>({ip:e}))}></lit-material-table>`}
      </section>

      ${e.authz?this.renderAuthz(e.authz,e.authzCheck):w}

      <section class="section" aria-labelledby="caches-title">
        <h2 id="caches-title">Response caches</h2>
        ${!e.caches||e.caches.length===0?x`<lit-material-empty-state size="compact" headline="No response cache">Add <code>[route.middleware.cache]</code> to a route; its cache appears here to purge.</lit-material-empty-state>`:x`<lit-material-text-field id="purge-path" variant="outlined" label="Path to purge" supporting-text="Empty purges everything; /products/* purges a prefix"></lit-material-text-field>
              <lit-material-table style="margin-top: var(--space-200)" caption="Response caches" caption-hidden density="compact" .columns=${[{key:`name`,label:`Cache`,mono:!0},{key:`kind`,label:`Store`},{key:`entries`,label:`Entries`,numeric:!0},{key:`bytes`,label:`On disk`,numeric:!0,render:e=>e.bytes===null?`—`:Ln(e.bytes)},{key:`hits`,label:`Hits / misses`,numeric:!0,render:e=>`${e.hits} / ${e.misses}`},{key:`action`,label:`Action`,hiddenLabel:!0,render:e=>x`<lit-material-button variant="text" ?disabled=${this.busy===`purge:${e.name}`} @click=${()=>this.purge(e.name)}><span slot="icon" class="icon">${Q(`trash`)}</span>Purge</lit-material-button>`}]} .rows=${e.caches}></lit-material-table>`}
      </section>

      <section class="section" aria-labelledby="flags-title">
        <h2 id="flags-title">Feature flags</h2>
        ${e.flags.length===0?x`<lit-material-empty-state size="compact" headline="No feature flags">Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</lit-material-empty-state>`:x`<lit-material-table caption="Feature flags" caption-hidden density="compact" .columns=${[{key:`name`,label:`Flag`,mono:!0},{key:`enabled`,label:`Enabled`,render:e=>x`<lit-material-switch aria-label="${e.name}" .checked=${e.enabled}
          @change=${t=>this.act(`flag:${e.name}`,`/flags/${encodeURIComponent(e.name)}`,{method:`PUT`,body:{enabled:t.target.checked}},e=>`${e.name} ${e.enabled?`on`:`off`}`)}></lit-material-switch>`}]} .rows=${e.flags}></lit-material-table>`}
      </section>
    `}renderAuthz(e,t){return x`
      <section class="section" aria-labelledby="authz-title">
        <h2 id="authz-title">Authorization</h2>
        <p class="lede">Relationship tuples, object#relation@subject. Schema: ${Object.entries(e.schema.types).filter(([,e])=>e.length).map(([e,t])=>`${e}: ${t.join(`, `)}`).join(` · `)}.</p>
        <form class="row" @submit=${e=>this.addTuple(e)} novalidate>
          <lit-material-text-field id="authz-tuple" variant="outlined" label="Tuple" supporting-text="e.g. doc:42#viewer@user:alice"></lit-material-text-field>
          <lit-material-button type="submit" ?disabled=${this.busy===`authz-add`}><span slot="icon" class="icon">${Q(`plus`)}</span>Add</lit-material-button>
        </form>
        ${e.tuples.length===0?x`<p class="lede" style="margin-top: var(--space-200)">No tuples yet.</p>`:x`<lit-material-table style="margin-top: var(--space-200)" caption="Relationship tuples" caption-hidden density="compact"
              .columns=${[{key:`tuple`,label:`Tuple`,mono:!0},{key:`action`,label:`Action`,hiddenLabel:!0,render:e=>x`<lit-material-button variant="text" ?disabled=${this.busy===`authz-del:${e.tuple}`}
          @click=${()=>this.act(`authz-del:${e.tuple}`,`/authz/tuples?tuple=${encodeURIComponent(e.tuple)}`,{method:`DELETE`},()=>`${e.tuple} removed`)}>Remove</lit-material-button>`}]} .rows=${e.tuples.map(e=>({tuple:e}))}></lit-material-table>`}
        <form class="row" style="margin-top: var(--space-300)" @submit=${e=>this.checkAccess(e)} novalidate>
          <lit-material-text-field id="authz-subject" variant="outlined" label="Subject" supporting-text="user:alice"></lit-material-text-field>
          <lit-material-text-field id="authz-relation" variant="outlined" label="Relation" supporting-text="viewer"></lit-material-text-field>
          <lit-material-text-field id="authz-object" variant="outlined" label="Object" supporting-text="doc:42"></lit-material-text-field>
          <lit-material-button type="submit" variant="tonal">Check</lit-material-button>
        </form>
        ${t?x`<p class="lede" id="authz-result" style="margin-top: var(--space-200)">
              ${t.allowed?$(`ok`,`Allowed`):$(`error`,`Denied`)}
              ${t.subject} ${t.relation} ${t.object}${t.allowed?x` — via <code>${t.path.join(` → `)}</code>`:w}</p>`:w}
      </section>
    `}renderConfig(){let e=this.data.config;if(!e)return x`<h1>Config</h1>${this.skeleton(2)}`;let t=this.filter.toLowerCase(),n=Object.entries(e).filter(([e,n])=>!t||e.toLowerCase().includes(t)||n.toLowerCase().includes(t)).map(([e,t])=>({setting:e,value:t}));return x`
      <h1>Config</h1>
      <p class="lede">Every setting in effect. Secrets read (hidden). Change them in rws.config.toml or the environment, then reload from Controls.</p>
      <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${e=>this.filter=e.target.value}></lit-material-text-field>
      ${n.length===0?x`<lit-material-empty-state size="compact" headline="No setting matches “${this.filter}”">Clear the filter to see all ${Object.keys(e).length}.</lit-material-empty-state>`:x`<lit-material-table caption="Settings in effect" caption-hidden density="compact" .columns=${[{key:`setting`,label:`Setting`,mono:!0},{key:`value`,label:`Value`,mono:!0,render:e=>e.value===``?$(`neutral`,`empty`):x`<lit-material-truncate position="middle" style="max-width: min(60ch, 50vw)" .value=${e.value}></lit-material-truncate>`}]} .rows=${n}></lit-material-table>`}
    `}renderAudit(){let e=this.data.audit;return e?x`
      <h1>Audit</h1>
      <p class="lede">Every change made through the admin API, the UI or MCP tools, and every refused attempt, newest first. Kept in memory (the last 1000); <code>[admin] audit_log</code> also writes them to a file.</p>
      ${e.length===0?x`<lit-material-empty-state size="compact" headline="No changes yet">Changes appear here as people reload, drain, block or purge.</lit-material-empty-state>`:x`<lit-material-table caption="Recent changes" caption-hidden density="compact" .columns=${[{key:`time`,label:`When`,render:e=>new Date(e.at*1e3).toLocaleString()},{key:`user`,label:`Who`,render:e=>x`${e.user} <span class="meta">${e.role}${e.via===`oidc`?` · SSO`:e.via===`mcp`?` · MCP`:``}</span>`},{key:`change`,label:`Change`,mono:!0,render:e=>`${e.method===`TOOL`?`tool`:e.method} ${e.path.replace(`/admin/api/v1`,``)}`},{key:`detail`,label:`Details`,mono:!0,render:e=>e.detail?x`<lit-material-truncate style="max-width: min(40ch, 30vw)" .value=${e.detail}></lit-material-truncate>`:w},{key:`status`,label:`Result`,render:e=>e.status<300?$(`ok`,`Done`):e.status===401||e.status===403?$(`error`,`Refused`):$(`warning`,String(e.status))},{key:`address`,label:`From`,mono:!0}]} .rows=${e}></lit-material-table>`}
    `:x`<h1>Audit</h1>${this.skeleton(2)}`}renderLogs(){return x`
      <h1>Logs</h1>
      <div class="row" style="margin-bottom: var(--space-200)">
        <p class="lede" style="margin: 0; flex: 1">The access log as it's written, newest at the bottom. Scroll up to read; it stops following until you're back at the bottom.</p>
        <lit-material-button variant="outlined" @click=${()=>this.paused=!this.paused}>
          <span slot="icon" class="icon">${Q(this.paused?`play`:`pause`)}</span>${this.paused?`Resume`:`Pause`}</lit-material-button>
      </div>
      <lit-material-log-viewer label="Access log" max-lines="1000" ?paused=${this.paused}></lit-material-log-viewer>
    `}};customElements.define(`rws-admin`,Dr);