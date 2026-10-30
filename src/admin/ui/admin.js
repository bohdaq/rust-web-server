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
`,M=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},N=class extends T{static{this.styles=et}constructor(){super(),this.variant=`standard`,this.position=`start`,this.selected=-1,this.open=!1,this.disableBackdropClose=!1,this.reopenOnClose=!1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected(),e.has(`open`)&&this.variant===`modal`&&this.dialogElement&&(this.open&&!this.dialogElement.open?this.dialogElement.showModal():!this.open&&this.dialogElement.open&&this.dialogElement.close())}show(){this.open=!0}close(){this.open=!1}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-drawer-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}renderNav(){let e=this.variant===`modal`;return x`
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
    `:this.renderNav()}handleClose(){this.reopenOnClose?(this.reopenOnClose=!1,this.dialogElement?.showModal()):(this.open=!1,this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0})))}handleCancel(e){this.dispatchEvent(new Event(`cancel`,{bubbles:!0,cancelable:!0,composed:!0}))||(e.preventDefault(),this.reopenOnClose=!0)}handleBackdropClick(e){!this.disableBackdropClose&&e.target===this.dialogElement&&this.dialogElement?.close()}};M([D()],N.prototype,`variant`,void 0),M([D({reflect:!0})],N.prototype,`position`,void 0),M([D({type:Number})],N.prototype,`selected`,void 0),M([D({type:Boolean,reflect:!0})],N.prototype,`open`,void 0),M([D({type:Boolean,attribute:`disable-backdrop-close`})],N.prototype,`disableBackdropClose`,void 0),M([k(`dialog`)],N.prototype,`dialogElement`,void 0),N=M([E(`lit-material-navigation-drawer`)],N);var tt=o`
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
`,nt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},P=class extends T{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Xe(this),this.focusRing=new Ze(this)}static{this.styles=tt}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return x`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};nt([D({type:Boolean,reflect:!0})],P.prototype,`selected`,void 0),nt([D({type:Boolean,reflect:!0})],P.prototype,`disabled`,void 0),nt([D()],P.prototype,`href`,void 0),nt([D()],P.prototype,`target`,void 0),nt([k(`.state-layer`)],P.prototype,`stateLayerElement`,void 0),P=nt([E(`lit-material-navigation-drawer-item`)],P);var rt=o`
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
`,it=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},at=class extends T{static{this.styles=rt}constructor(){super(),this.alignment=`top`,this.selected=-1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected()}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-rail-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}render(){return x`
      <div class="rail" part="rail">
        <div class="fab" part="fab"><slot name="fab"></slot></div>
        <div class="items ${this.alignment}" part="items">
          <slot @slotchange=${this.handleSlotChange}></slot>
        </div>
      </div>
    `}};it([D()],at.prototype,`alignment`,void 0),it([D({type:Number})],at.prototype,`selected`,void 0),at=it([E(`lit-material-navigation-rail`)],at);var ot=o`
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
`,st=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},F=class extends T{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Xe(this),this.focusRing=new Ze(this)}static{this.styles=ot}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return x`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};st([D({type:Boolean,reflect:!0})],F.prototype,`selected`,void 0),st([D({type:Boolean,reflect:!0})],F.prototype,`disabled`,void 0),st([D()],F.prototype,`href`,void 0),st([D()],F.prototype,`target`,void 0),st([k(`.state-layer`)],F.prototype,`stateLayerElement`,void 0),F=st([E(`lit-material-navigation-rail-item`)],F);var ct=o`
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
`,lt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ut=class extends T{static{this.styles=ct}constructor(){super(),this.selected=-1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected()}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-bar-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}render(){return x`
      <div class="bar" part="bar">
        <slot @slotchange=${this.handleSlotChange}></slot>
      </div>
    `}};lt([D({type:Number})],ut.prototype,`selected`,void 0),ut=lt([E(`lit-material-navigation-bar`)],ut);var dt=o`
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
`,ft=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},I=class extends T{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Xe(this),this.focusRing=new Ze(this)}static{this.styles=dt}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return x`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};ft([D({type:Boolean,reflect:!0})],I.prototype,`selected`,void 0),ft([D({type:Boolean,reflect:!0})],I.prototype,`disabled`,void 0),ft([D()],I.prototype,`href`,void 0),ft([D()],I.prototype,`target`,void 0),ft([k(`.state-layer`)],I.prototype,`stateLayerElement`,void 0),I=ft([E(`lit-material-navigation-bar-item`)],I);var pt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},mt=class extends T{constructor(){super(...arguments),this.destinations=[],this.selected=-1,this.expanded=`drawer`,this.label=`Main`}static{this.styles=o`
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
    `}};pt([D({attribute:!1})],mt.prototype,`destinations`,void 0),pt([D({type:Number})],mt.prototype,`selected`,void 0),pt([D({reflect:!0})],mt.prototype,`expanded`,void 0),pt([D()],mt.prototype,`label`,void 0),mt=pt([E(`lit-material-adaptive-navigation`)],mt);var ht=o`
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
`,gt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},_t={ok:S`<circle cx="10" cy="10" r="8"/><path d="M6.5 10.2l2.4 2.4 4.6-4.8"/>`,warning:S`<path d="M10 3l7.5 13h-15z"/><path d="M10 8.2v3.6M10 14.2v.1"/>`,error:S`<circle cx="10" cy="10" r="8"/><path d="M7.2 7.2l5.6 5.6M12.8 7.2l-5.6 5.6"/>`,info:S`<circle cx="10" cy="10" r="8"/><path d="M10 9v5M10 6.2v.1"/>`,neutral:S`<circle cx="10" cy="10" r="3.2" class="fill"/>`},vt=new Set([`ok`,`warning`,`error`,`info`,`neutral`]),yt=class extends T{constructor(){super(...arguments),this.status=`neutral`}static{this.styles=ht}get kind(){return vt.has(this.status)?this.status:`neutral`}render(){let e=this.kind;return x`<span class="status ${e}" part="status">
      <span class="icon" part="icon" aria-hidden="true"
        ><slot name="icon"><svg viewBox="0 0 20 20" focusable="false">${_t[e]}</svg></slot></span
      ><span class="label"><slot></slot></span>
    </span>`}};gt([D({reflect:!0})],yt.prototype,`status`,void 0),yt=gt([E(`lit-material-status`)],yt);var bt=o`
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
`,L=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},R=class extends T{constructor(){super(...arguments),this.label=``,this.value=``,this.caption=``,this.trendLabel=``,this.trendSentiment=`neutral`,this.variant=`outlined`}static{this.styles=bt}arrow(){switch(this.trend){case`up`:return`↑`;case`down`:return`↓`;case`flat`:return`→`;default:return``}}render(){let e=this.trend&&this.trendLabel;return x`
      <div class="stat ${this.variant}" part="stat" role="group" aria-label=${this.label||w}>
        <div class="label" part="label"><slot name="icon"></slot><span>${this.label}</span></div>
        <div class="value" part="value">${this.value}</div>
        <div class="caption" part="caption">
          ${e?x`<span class="trend ${this.trendSentiment}"><span aria-hidden="true">${this.arrow()}</span> ${this.trendLabel}</span>`:w}
          <slot name="caption">${this.caption?x`<span>${this.caption}</span>`:w}</slot>
        </div>
      </div>
    `}};L([D()],R.prototype,`label`,void 0),L([D()],R.prototype,`value`,void 0),L([D()],R.prototype,`caption`,void 0),L([D()],R.prototype,`trend`,void 0),L([D({attribute:`trend-label`})],R.prototype,`trendLabel`,void 0),L([D({attribute:`trend-sentiment`})],R.prototype,`trendSentiment`,void 0),L([D({reflect:!0})],R.prototype,`variant`,void 0),R=L([E(`lit-material-stat`)],R);var xt=o`
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
`,St=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},z=class extends T{constructor(){super(...arguments),this.headline=``,this.level=2,this.size=`standard`,this.emptyIcon=!1,this.emptyActions=!1}static{this.styles=xt}onSlotChange(e){let t=e.target,n=t.assignedNodes({flatten:!0}).length===0;t.name===`icon`?this.emptyIcon=n:this.emptyActions=n}firstUpdated(){for(let e of this.renderRoot.querySelectorAll(`slot[name]`))e.dispatchEvent(new Event(`slotchange`))}render(){let e=Math.min(6,Math.max(1,Math.round(this.level||2)));return x`
      <div class="empty-state ${this.size}" part="empty-state">
        <div class="icon ${this.emptyIcon?`empty`:``}"><slot name="icon" @slotchange=${this.onSlotChange}></slot></div>
        ${this.headline?x`<div class="headline" part="headline" role="heading" aria-level=${e}>${this.headline}</div>`:w}
        <div class="body"><slot></slot></div>
        <div class="actions ${this.emptyActions?`empty`:``}"><slot name="actions" @slotchange=${this.onSlotChange}></slot></div>
      </div>
    `}};St([D()],z.prototype,`headline`,void 0),St([D({type:Number})],z.prototype,`level`,void 0),St([D({reflect:!0})],z.prototype,`size`,void 0),St([O()],z.prototype,`emptyIcon`,void 0),St([O()],z.prototype,`emptyActions`,void 0),z=St([E(`lit-material-empty-state`)],z);var Ct={ATTRIBUTE:1,CHILD:2,PROPERTY:3,BOOLEAN_ATTRIBUTE:4,EVENT:5,ELEMENT:6},wt=e=>(...t)=>({_$litDirective$:e,values:t}),Tt=class{constructor(e){}get _$AU(){return this._$AM._$AU}_$AT(e,t,n){this._$Ct=e,this._$AM=t,this._$Ci=n}_$AS(e,t){return this.update(e,t)}update(e,t){return this.render(...t)}},{I:Et}=Re,Dt=e=>e,Ot=()=>document.createComment(``),kt=(e,t,n)=>{let r=e._$AA.parentNode,i=t===void 0?e._$AB:t._$AA;if(n===void 0)n=new Et(r.insertBefore(Ot(),i),r.insertBefore(Ot(),i),e,e.options);else{let t=n._$AB.nextSibling,a=n._$AM,o=a!==e;if(o){let t;n._$AQ?.(e),n._$AM=e,n._$AP!==void 0&&(t=e._$AU)!==a._$AU&&n._$AP(t)}if(t!==i||o){let e=n._$AA;for(;e!==t;){let t=Dt(e).nextSibling;Dt(r).insertBefore(e,i),e=t}}}return n},B=(e,t,n=e)=>(e._$AI(t,n),e),At={},jt=(e,t=At)=>e._$AH=t,Mt=e=>e._$AH,Nt=e=>{e._$AR(),e._$AA.remove()},Pt=(e,t,n)=>{let r=new Map;for(let i=t;i<=n;i++)r.set(e[i],i);return r},Ft=wt(class extends Tt{constructor(e){if(super(e),e.type!==Ct.CHILD)throw Error(`repeat() can only be used in text expressions`)}dt(e,t,n){let r;n===void 0?n=t:t!==void 0&&(r=t);let i=[],a=[],o=0;for(let t of e)i[o]=r?r(t,o):o,a[o]=n(t,o),o++;return{values:a,keys:i}}render(e,t,n){return this.dt(e,t,n).values}update(e,[t,n,r]){let i=Mt(e),{values:a,keys:o}=this.dt(t,n,r);if(!Array.isArray(i))return this.ut=o,a;let s=this.ut??=[],c=[],l,u,d=0,f=i.length-1,p=0,m=a.length-1;for(;d<=f&&p<=m;)if(i[d]===null)d++;else if(i[f]===null)f--;else if(s[d]===o[p])c[p]=B(i[d],a[p]),d++,p++;else if(s[f]===o[m])c[m]=B(i[f],a[m]),f--,m--;else if(s[d]===o[m])c[m]=B(i[d],a[m]),kt(e,c[m+1],i[d]),d++,m--;else if(s[f]===o[p])c[p]=B(i[f],a[p]),kt(e,i[d],i[f]),f--,p++;else if(l===void 0&&(l=Pt(o,p,m),u=Pt(s,d,f)),l.has(s[d])){if(l.has(s[f])){let t=u.get(o[p]),n=t===void 0?null:i[t];if(n===null){let t=kt(e,i[d]);B(t,a[p]),c[p]=t}else c[p]=B(n,a[p]),kt(e,i[d],n),i[t]=null;p++}else Nt(i[f]),f--}else Nt(i[d]),d++;for(;p<=m;){let t=kt(e,c[m+1]);B(t,a[p]),c[p++]=t}for(;d<=f;){let e=i[d++];e!==null&&Nt(e)}return this.ut=o,jt(e,c),C}}),It=o`
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
`,V=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},H=class extends T{constructor(){super(...arguments),this.maxLines=1e3,this.paused=!1,this.wrap=!1,this.label=`Log`,this.entries=[],this.unseen=0,this.held=[],this.nextId=0,this.following=!0}static{this.styles=It}get lines(){return this.entries.map(e=>e.text)}set lines(e){this.entries=[],this.held=[],this.unseen=0,this.following=!0,this.add(e)}appendLines(e){let t=(Array.isArray(e)?e:[e]).flatMap(e=>String(e).split(`
`));this.paused?this.held=[...this.held,...t].slice(-this.maxLines):this.add(t)}clear(){this.entries=[],this.held=[],this.unseen=0}scrollToLatest(){this.following=!0,this.unseen=0;let e=this.viewport;e&&(e.scrollTop=e.scrollHeight)}add(e){if(e.length===0)return;let t=e.map(e=>({id:this.nextId++,text:e,level:this.highlight?.(e)??void 0})),n=[...this.entries,...t],r=Math.max(0,n.length-this.maxLines);this.entries=r?n.slice(r):n,r&&this.dispatchEvent(new CustomEvent(`lines-dropped`,{detail:{count:r},bubbles:!0,composed:!0})),this.following||(this.unseen+=t.length)}willUpdate(e){if(e.has(`paused`)&&!this.paused&&this.held.length){let e=this.held;this.held=[],this.add(e)}}updated(){this.following&&this.viewport&&(this.viewport.scrollTop=this.viewport.scrollHeight)}onScroll(){let e=this.viewport;e&&(this.following=e.scrollHeight-e.scrollTop-e.clientHeight<24,this.following&&(this.unseen=0))}render(){return x`
      <div class="box">
        <div class="viewport" part="viewport" role="log" aria-live="off" aria-label=${this.label} tabindex="0" @scroll=${this.onScroll}>
          ${Ft(this.entries,e=>e.id,e=>x`<div class="line ${e.level??``}" part="line">${e.level?x`<span class="mark" aria-label=${e.level}></span>`:w}${e.text}</div>`)}
        </div>
        ${this.unseen>0?x`<button class="jump" part="jump" @click=${()=>this.scrollToLatest()}>${this.unseen} new line${this.unseen===1?``:`s`} ↓</button>`:w}
        ${this.paused&&this.held.length?x`<div class="held" role="status">Paused: ${this.held.length} line${this.held.length===1?``:`s`} waiting</div>`:w}
      </div>
    `}};V([D({type:Number,attribute:`max-lines`})],H.prototype,`maxLines`,void 0),V([D({type:Boolean,reflect:!0})],H.prototype,`paused`,void 0),V([D({type:Boolean,reflect:!0})],H.prototype,`wrap`,void 0),V([D()],H.prototype,`label`,void 0),V([D({attribute:!1})],H.prototype,`highlight`,void 0),V([O()],H.prototype,`entries`,void 0),V([O()],H.prototype,`unseen`,void 0),V([O()],H.prototype,`held`,void 0),V([k(`.viewport`)],H.prototype,`viewport`,void 0),H=V([E(`lit-material-log-viewer`)],H);var Lt=o`
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
`,Rt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},U=class extends T{constructor(){super(...arguments),this.columns=[],this.rows=[],this.caption=``,this.captionHidden=!1,this.density=`standard`,this.stickyHeader=!1}static{this.styles=Lt}cell(e,t){if(t.render)return t.render(e);let n=e[t.key];return n==null?``:String(n)}render(){let e=this.columns;return x`
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
    `}};Rt([D({attribute:!1})],U.prototype,`columns`,void 0),Rt([D({attribute:!1})],U.prototype,`rows`,void 0),Rt([D()],U.prototype,`caption`,void 0),Rt([D({type:Boolean,attribute:`caption-hidden`})],U.prototype,`captionHidden`,void 0),Rt([D({reflect:!0})],U.prototype,`density`,void 0),Rt([D({type:Boolean,reflect:!0,attribute:`sticky-header`})],U.prototype,`stickyHeader`,void 0),U=Rt([E(`lit-material-table`)],U);function zt(e,t){if(!(e>0)||!Number.isFinite(e))return 1;let n=e/Math.max(1,t),r=10**Math.floor(Math.log10(n));for(let e of[1,2,2.5,5,10])if(e*r>=n)return e*r;return 10*r}function Bt(e,t,n=4){if(!Number.isFinite(e)||!Number.isFinite(t))return{ticks:[0,1],min:0,max:1};if(e===t){let n=e===0?1:Math.abs(e)*.1;e-=n,t+=n}let r=zt(t-e,n),i=Math.floor(e/r)*r,a=Math.ceil(t/r)*r,o=[];for(let e=i,t=0;e<=a+r/2&&t<n*3+2;e+=r,t++)o.push(Number(e.toPrecision(12)));return{ticks:o,min:i,max:a}}function Vt(e,t,n=5){return!Number.isFinite(e)||!Number.isFinite(t)?[]:e===t?[e]:Array.from({length:n},(r,i)=>e+(t-e)*i/(n-1))}function Ht(e){if(!Number.isFinite(e))return``;let t=Math.abs(e);return t>=1e9?`${Ut(e/1e9)}G`:t>=1e6?`${Ut(e/1e6)}M`:t>=1e4?`${Ut(e/1e3)}k`:t>=100||Number.isInteger(e)?String(Math.round(e)):Ut(e)}function Ut(e){return String(Number(e.toPrecision(3)))}function Wt(e,t,n,r){let i=e.map((e,t)=>({p:e,i:t})).sort((e,t)=>e.p-t.p),a=[...e],o=-1/0;for(let{p:e,i:r}of i){let i=Math.max(e,o+t,n);a[r]=i,o=i}let s=Math.max(0,Math.max(...a)-r);return s?a.map(e=>e-s):a}var Gt=a([[`#2a78d6`,`#3987e5`],[`#eb6834`,`#d95926`],[`#1baf7a`,`#199e70`],[`#eda100`,`#c98500`],[`#e87ba4`,`#d55181`],[`#008300`,`#008300`],[`#6250d6`,`#9085e9`],[`#e34948`,`#e66767`]].map(([e,t],n)=>`.s${n+1} { --series: var(--lit-material-chart-series-${n+1}, light-dark(${e}, ${t})); }`).join(`
`)),Kt=o`
  :host {
    --chart-text: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    --chart-muted: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    --chart-grid: var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    --chart-surface: var(--md-sys-color-surface, light-dark(#fef7ff, #141218));
  }
`,qt=[Kt,Gt,o`
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
  `],W=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Jt=4,Yt=8,Xt=28,Zt=40,G=class extends T{constructor(){super(...arguments),this.series=[],this.label=``,this.height=240,this.formatX=Ht,this.formatY=Ht,this.unit=``,this.width=640,this.active=-1}static{this.styles=qt}connectedCallback(){super.connectedCallback(),typeof ResizeObserver<`u`&&(this.observer=new ResizeObserver(e=>{let t=Math.round(e[0]?.contentRect.width??0);t>0&&t!==this.width&&requestAnimationFrame(()=>this.width=t)}),this.observer.observe(this))}disconnectedCallback(){super.disconnectedCallback(),this.observer?.disconnect()}get drawn(){return this.series.length>8&&console.warn(`lit-material-line-chart: ${this.series.length} series; only the first 8 are drawn`),this.series.slice(0,8).map(e=>({name:e.name,points:e.points.filter(e=>Number.isFinite(e.x)&&Number.isFinite(e.y)).sort((e,t)=>e.x-t.x)}))}xsOf(e){let t=new Set;for(let n of e)for(let e of n.points)t.add(e.x);return[...t].sort((e,t)=>e-t)}nearest(e,t){let n;for(let r of e)(!n||Math.abs(r.x-t)<Math.abs(n.x-t))&&(n=r);return n}onPointer(e,t,n){let r=e.currentTarget.getBoundingClientRect(),i=(e.clientX-r.left)/Math.max(1,r.width)*this.width,a=0;for(let e=1;e<t.length;e++)Math.abs(n(t[e])-i)<Math.abs(n(t[a])-i)&&(a=e);this.active=a}onKey(e,t){let n={ArrowLeft:-1,ArrowRight:1,Home:-1/0,End:1/0};if(e.key===`Escape`){this.active=-1;return}if(!(e.key in n))return;e.preventDefault();let r=n[e.key],i=this.active<0?t-1:this.active;this.active=Math.min(t-1,Math.max(0,Number.isFinite(r)?i+r:r<0?0:t-1))}summary(e){return e.map(e=>{let t=e.points.map(e=>e.y);if(!t.length)return`${e.name}: no data`;let n=t[t.length-1];return`${e.name}: latest ${this.formatY(n)}${this.unit?` ${this.unit}`:``}, range ${this.formatY(Math.min(...t))} to ${this.formatY(Math.max(...t))}`}).join(`; `)}render(){let e=this.drawn,t=this.xsOf(e),n=this.label||`Line chart`;if(t.length===0)return x`<div class="empty" role="img" aria-label="${n}: no data">No data</div>`;let r=e.flatMap(e=>e.points.map(e=>e.y)),i=Math.min(...r),a=this.yMin??Math.min(0,i),{ticks:o,min:s,max:c}=Bt(Math.min(a,i),Math.max(...r),4),l=e.length>1&&e.length<=Jt,u=Math.max(0,...e.map(e=>e.name.length)),d=l?Math.min(160,12+u*7):12,f=o.map(e=>this.formatY(e)),p=Math.max(Zt,12+Math.max(...f.map(e=>e.length))*7),m=this.width,h=Math.max(80,this.height),ee=Math.max(1,m-p-d),te=Math.max(1,h-Yt-Xt),ne=t[0],g=t[t.length-1],_=e=>p+(g===ne?ee/2:(e-ne)/(g-ne)*ee),v=e=>Yt+(1-(e-s)/(c-s||1))*te,re=Vt(ne,g,Math.max(2,Math.min(6,Math.floor(ee/110)+1))),ie=e.map(e=>e.points.length?v(e.points[e.points.length-1].y):Yt),ae=l?Wt(ie,14,14,Yt+te):[],y=this.active>=0&&this.active<t.length?t[this.active]:void 0,oe=y===void 0?[]:e.map((e,t)=>({i:t,name:e.name,point:this.nearest(e.points,y)})).filter(e=>e.point),se=y===void 0?0:_(y)/m*100;return x`
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
          ${l?e.map((e,t)=>S`<text class="end-label" x=${p+ee+6} y=${(ae[t]??Yt)+4}>${e.name}</text>`):w}
          ${y===void 0?w:S`<line class="crosshair" x1=${_(y)} x2=${_(y)} y1=${Yt} y2=${Yt+te}></line>
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
    `}};W([D({attribute:!1})],G.prototype,`series`,void 0),W([D()],G.prototype,`label`,void 0),W([D({type:Number})],G.prototype,`height`,void 0),W([D({type:Number,attribute:`y-min`})],G.prototype,`yMin`,void 0),W([D({attribute:!1})],G.prototype,`formatX`,void 0),W([D({attribute:!1})],G.prototype,`formatY`,void 0),W([D()],G.prototype,`unit`,void 0),W([O()],G.prototype,`width`,void 0),W([O()],G.prototype,`active`,void 0),G=W([E(`lit-material-line-chart`)],G);var Qt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},$t=class extends T{constructor(){super(...arguments),this.values=[],this.label=``,this.showLast=!1,this.format=Ht}static{this.styles=[Kt,Gt,o`
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
    </svg>`}};Qt([D({attribute:!1})],$t.prototype,`values`,void 0),Qt([D()],$t.prototype,`label`,void 0),Qt([D({type:Boolean,attribute:`show-last`})],$t.prototype,`showLast`,void 0),Qt([D({attribute:!1})],$t.prototype,`format`,void 0),$t=Qt([E(`lit-material-sparkline`)],$t);var en=o`
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
`,tn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},nn=class extends T{constructor(){super(...arguments),this.variant=`text`,this.animation=`pulse`,this.width=``,this.height=``}static{this.styles=en}render(){return x`<span class="skeleton" part="skeleton" style=${`${this.width?`width: ${this.width};`:``}${this.height?`height: ${this.height};`:``}`||w} aria-hidden="true"></span>`}};tn([D({reflect:!0})],nn.prototype,`variant`,void 0),tn([D({reflect:!0})],nn.prototype,`animation`,void 0),tn([D()],nn.prototype,`width`,void 0),tn([D()],nn.prototype,`height`,void 0),nn=tn([E(`lit-material-skeleton`)],nn);var rn=o`
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
`,an=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},on=class extends T{static{this.styles=rn}constructor(){super(),this.open=!1,this.duration=5e3,this.closable=!1,this.pauseTimer=()=>{this.open&&this.clearTimer()},this.resumeTimer=()=>{this.open&&this.startTimer()},this.handleClick=e=>{e.target.closest(`[slot="action"]`)&&this.close()},this.addEventListener(`click`,this.handleClick),this.addEventListener(`pointerenter`,this.pauseTimer),this.addEventListener(`pointerleave`,this.resumeTimer),this.addEventListener(`focusin`,this.pauseTimer),this.addEventListener(`focusout`,this.resumeTimer)}connectedCallback(){super.connectedCallback(),this.setAttribute(`popover`,`manual`),this.setAttribute(`role`,`status`),this.setAttribute(`aria-live`,`polite`),this.setAttribute(`aria-atomic`,`true`)}disconnectedCallback(){super.disconnectedCallback(),this.clearTimer()}show(e){if(e!==void 0){for(let e of[...this.childNodes])e.nodeType===Node.TEXT_NODE&&e.remove();this.prepend(document.createTextNode(e))}this.open=!0}close(){this.open=!1}updated(e){e.has(`open`)&&(this.open?(this.matches(`:popover-open`)||this.showPopover(),this.startTimer()):this.matches(`:popover-open`)&&(this.clearTimer(),this.hidePopover(),this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0}))))}render(){return x`
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
    `}startTimer(){this.clearTimer(),this.duration>0&&(this.timeoutId=window.setTimeout(()=>this.close(),this.duration))}clearTimer(){this.timeoutId!==void 0&&(window.clearTimeout(this.timeoutId),this.timeoutId=void 0)}};an([D({type:Boolean,reflect:!0})],on.prototype,`open`,void 0),an([D({type:Number})],on.prototype,`duration`,void 0),an([D({type:Boolean})],on.prototype,`closable`,void 0),on=an([E(`lit-material-snackbar`)],on);var sn=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},cn=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}},ln=o`
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
`,K=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},q=class extends T{static{this.styles=ln}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.checked=!1,this.disabled=!1,this.error=!1,this.required=!1,this.name=``,this.value=`on`,this.ariaLabel=null,this.ariaLabelledBy=null,this.ripple=new sn(this),this.focusRing=new cn(this),this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement),this.syncInternals()}updated(e){(e.has(`checked`)||e.has(`required`)||e.has(`error`))&&this.syncInternals()}syncInternals(){if(this.internals.setFormValue(this.checked?this.value:null),this.inputElement){let e=this.error?this.inputElement.validationMessage||`Invalid`:this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){return x`
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
    `}handleChange(e){let t=e.target;this.checked=t.checked,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}formResetCallback(){this.checked=this.hasAttribute(`checked`),this.syncInternals()}formStateRestoreCallback(e){this.checked=e!==null}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};K([D({type:Boolean})],q.prototype,`checked`,void 0),K([D({type:Boolean,reflect:!0})],q.prototype,`disabled`,void 0),K([D({type:Boolean,reflect:!0})],q.prototype,`error`,void 0),K([D({type:Boolean})],q.prototype,`required`,void 0),K([D()],q.prototype,`name`,void 0),K([D()],q.prototype,`value`,void 0),K([D({reflect:!0})],q.prototype,`form`,void 0),K([D({attribute:`aria-label`})],q.prototype,`ariaLabel`,void 0),K([D({attribute:`aria-labelledby`})],q.prototype,`ariaLabelledBy`,void 0),K([k(`.native-control`)],q.prototype,`inputElement`,void 0),K([k(`.state-layer`)],q.prototype,`stateLayerElement`,void 0),q=K([E(`lit-material-switch`)],q);var un=o`
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
`,J=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},dn=0,Y=class extends T{static{this.styles=un}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`text`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.prefix=``,this.suffix=``,this.step=``,this.pattern=``,this.autocomplete=``,this.inputmode=``,this.name=``,this.focused=!1,this.touched=!1,this.focusRing=new Ze(this),this.labelId=`lit-material-text-field-label-${++dn}`,this.supportingId=`lit-material-text-field-supporting-${dn}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`pattern`)||e.has(`minlength`)||e.has(`maxlength`)||e.has(`min`)||e.has(`max`)||e.has(`type`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.inputElement?!1:!this.inputElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.inputElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.inputElement){let e=this.isError&&this.errorText||this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){let e=this.floated,t=this.isError;return x`
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
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleKeydown(e){if(e.key!==`Enter`||e.isComposing||e.defaultPrevented)return;let t=this.internals.form;t&&(e.preventDefault(),t.requestSubmit())}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};J([D()],Y.prototype,`variant`,void 0),J([D()],Y.prototype,`type`,void 0),J([D()],Y.prototype,`value`,void 0),J([D()],Y.prototype,`label`,void 0),J([D()],Y.prototype,`placeholder`,void 0),J([D({attribute:`supporting-text`})],Y.prototype,`supportingText`,void 0),J([D({attribute:`error-text`})],Y.prototype,`errorText`,void 0),J([D({type:Boolean,reflect:!0})],Y.prototype,`error`,void 0),J([D({type:Boolean,reflect:!0})],Y.prototype,`disabled`,void 0),J([D({type:Boolean})],Y.prototype,`required`,void 0),J([D({type:Boolean})],Y.prototype,`readonly`,void 0),J([D()],Y.prototype,`prefix`,void 0),J([D()],Y.prototype,`suffix`,void 0),J([D({type:Number})],Y.prototype,`min`,void 0),J([D({type:Number})],Y.prototype,`max`,void 0),J([D({type:Number})],Y.prototype,`minlength`,void 0),J([D({type:Number})],Y.prototype,`maxlength`,void 0),J([D()],Y.prototype,`step`,void 0),J([D()],Y.prototype,`pattern`,void 0),J([D()],Y.prototype,`autocomplete`,void 0),J([D()],Y.prototype,`inputmode`,void 0),J([D()],Y.prototype,`name`,void 0),J([D({reflect:!0})],Y.prototype,`form`,void 0),J([k(`.input`)],Y.prototype,`inputElement`,void 0),J([O()],Y.prototype,`focused`,void 0),J([O()],Y.prototype,`touched`,void 0),Y=J([E(`lit-material-text-field`)],Y);var fn=o`
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
`,X=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},pn=0,Z=class extends T{static{this.styles=fn}static{this.shadowRootOptions={...T.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.rows=4,this.resize=`vertical`,this.autocomplete=``,this.name=``,this.focused=!1,this.touched=!1,this.labelId=`lit-material-textarea-label-${++pn}`,this.supportingId=`lit-material-textarea-supporting-${pn}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`minlength`)||e.has(`maxlength`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.textareaElement?!1:!this.textareaElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.textareaElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.textareaElement){let e=this.isError&&this.errorText||this.textareaElement.validationMessage;this.internals.setValidity(this.textareaElement.validity,e,this.textareaElement)}}render(){let e=this.floated,t=this.isError;return x`
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
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};X([D()],Z.prototype,`variant`,void 0),X([D()],Z.prototype,`value`,void 0),X([D()],Z.prototype,`label`,void 0),X([D()],Z.prototype,`placeholder`,void 0),X([D({attribute:`supporting-text`})],Z.prototype,`supportingText`,void 0),X([D({attribute:`error-text`})],Z.prototype,`errorText`,void 0),X([D({type:Boolean,reflect:!0})],Z.prototype,`error`,void 0),X([D({type:Boolean,reflect:!0})],Z.prototype,`disabled`,void 0),X([D({type:Boolean})],Z.prototype,`required`,void 0),X([D({type:Boolean})],Z.prototype,`readonly`,void 0),X([D({type:Number})],Z.prototype,`minlength`,void 0),X([D({type:Number})],Z.prototype,`maxlength`,void 0),X([D({type:Number})],Z.prototype,`rows`,void 0),X([D()],Z.prototype,`resize`,void 0),X([D()],Z.prototype,`autocomplete`,void 0),X([D()],Z.prototype,`name`,void 0),X([D({reflect:!0})],Z.prototype,`form`,void 0),X([k(`.textarea`)],Z.prototype,`textareaElement`,void 0),X([O()],Z.prototype,`focused`,void 0),X([O()],Z.prototype,`touched`,void 0),Z=X([E(`lit-material-textarea`)],Z);var mn=`rws-admin-token`,hn=``;function gn(){try{return sessionStorage.getItem(mn)??hn}catch{return hn}}function _n(e){hn=e;try{e?sessionStorage.setItem(mn,e):sessionStorage.removeItem(mn)}catch{}}var vn=class extends Error{constructor(e,t){super(t),this.status=e}};async function yn(e,{method:t=`GET`,body:n}={}){let r=await fetch(`/admin/api/v1${e}`,{method:t,headers:{Authorization:`Bearer ${gn()}`,...n===void 0?{}:{"Content-Type":`application/json`}},body:n===void 0?void 0:JSON.stringify(n),cache:`no-store`}),i=await r.text(),a=null;try{a=i?JSON.parse(i):null}catch{a=null}if(!r.ok)throw new vn(r.status,a?.error??`${r.status} ${r.statusText}`);return a}async function bn(e,t){let n=await fetch(`/admin/api/v1/logs/stream`,{headers:{Authorization:`Bearer ${gn()}`},signal:t,cache:`no-store`});if(!n.ok||!n.body)throw new vn(n.status,`${n.status} ${n.statusText}`);let r=n.body.pipeThrough(new TextDecoderStream).getReader(),i=``;for(;;){let{value:t,done:n}=await r.read();if(n)return;i+=t;let a;for(;(a=i.indexOf(`

`))>=0;){let t=i.slice(0,a);if(i=i.slice(a+2),t.split(`
`).some(e=>e.startsWith(`event:`)))continue;let n=t.split(`
`).filter(e=>e.startsWith(`data:`)).map(e=>e.slice(5).replace(/^ /,``)).join(`
`);n&&e(n)}}}var xn=class extends Tt{constructor(e){if(super(e),this.it=w,e.type!==Ct.CHILD)throw Error(this.constructor.directiveName+`() can only be used in child bindings`)}render(e){if(e===w||e==null)return this._t=void 0,this.it=e;if(e===C)return e;if(typeof e!=`string`)throw Error(this.constructor.directiveName+`() called with a non-string value`);if(e===this.it)return this._t;this.it=e;let t=[e];return t.raw=t,this._t={_$litType$:this.constructor.resultType,strings:t,values:[]}}};xn.directiveName=`unsafeHTML`,xn.resultType=1;var Sn=class extends xn{};Sn.directiveName=`unsafeSVG`,Sn.resultType=2;var Cn=wt(Sn),wn={gauge:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M207.06,72.67A111.24,111.24,0,0,0,128,40h-.4C66.07,40.21,16,91,16,153.13V176a16,16,0,0,0,16,16H224a16,16,0,0,0,16-16V152A111.25,111.25,0,0,0,207.06,72.67ZM224,176H119.71l54.76-75.3a8,8,0,0,0-12.94-9.42L99.92,176H32V153.13c0-3.08.15-6.12.43-9.13H56a8,8,0,0,0,0-16H35.27c10.32-38.86,44-68.24,84.73-71.66V80a8,8,0,0,0,16,0V56.33A96.14,96.14,0,0,1,221,128H200a8,8,0,0,0,0,16h23.67c.21,2.65.33,5.31.33,8Z"/></svg>`,"tree-structure":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M160,112h48a16,16,0,0,0,16-16V48a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16V64H128a24,24,0,0,0-24,24v32H72v-8A16,16,0,0,0,56,96H24A16,16,0,0,0,8,112v32a16,16,0,0,0,16,16H56a16,16,0,0,0,16-16v-8h32v32a24,24,0,0,0,24,24h16v16a16,16,0,0,0,16,16h48a16,16,0,0,0,16-16V160a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16v16H128a8,8,0,0,1-8-8V88a8,8,0,0,1,8-8h16V96A16,16,0,0,0,160,112ZM56,144H24V112H56v32Zm104,16h48v48H160Zm0-112h48V96H160Z"/></svg>`,certificate:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,136a8,8,0,0,1-8,8H72a8,8,0,0,1,0-16h48A8,8,0,0,1,128,136Zm-8-40H72a8,8,0,0,0,0,16h48a8,8,0,0,0,0-16Zm112,65.47V224A8,8,0,0,1,220,231l-24-13.74L172,231A8,8,0,0,1,160,224V200H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216a16,16,0,0,1,16,16V86.53a51.88,51.88,0,0,1,0,74.94ZM160,184V161.47A52,52,0,0,1,216,76V56H40V184Zm56-12a51.88,51.88,0,0,1-40,0v38.22l16-9.16a8,8,0,0,1,7.94,0l16,9.16Zm16-48a36,36,0,1,0-36,36A36,36,0,0,0,232,124Z"/></svg>`,"sliders-horizontal":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M40,88H73a32,32,0,0,0,62,0h81a8,8,0,0,0,0-16H135a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16Zm64-24A16,16,0,1,1,88,80,16,16,0,0,1,104,64ZM216,168H199a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16h97a32,32,0,0,0,62,0h17a8,8,0,0,0,0-16Zm-48,24a16,16,0,1,1,16-16A16,16,0,0,1,168,192Z"/></svg>`,"list-bullets":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M80,64a8,8,0,0,1,8-8H216a8,8,0,0,1,0,16H88A8,8,0,0,1,80,64Zm136,56H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16Zm0,64H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16ZM44,52A12,12,0,1,0,56,64,12,12,0,0,0,44,52Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,116Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,180Z"/></svg>`,"terminal-window":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,128a8,8,0,0,1-3,6.25l-40,32a8,8,0,1,1-10-12.5L107.19,128,75,102.25a8,8,0,1,1,10-12.5l40,32A8,8,0,0,1,128,128Zm48,24H136a8,8,0,0,0,0,16h40a8,8,0,0,0,0-16Zm56-96V200a16,16,0,0,1-16,16H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216A16,16,0,0,1,232,56ZM216,200V56H40V200H216Z"/></svg>`,"sign-out":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M120,216a8,8,0,0,1-8,8H48a8,8,0,0,1-8-8V40a8,8,0,0,1,8-8h64a8,8,0,0,1,0,16H56V208h56A8,8,0,0,1,120,216Zm109.66-93.66-40-40a8,8,0,0,0-11.32,11.32L204.69,120H112a8,8,0,0,0,0,16h92.69l-26.35,26.34a8,8,0,0,0,11.32,11.32l40-40A8,8,0,0,0,229.66,122.34Z"/></svg>`,"check-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M173.66,98.34a8,8,0,0,1,0,11.32l-56,56a8,8,0,0,1-11.32,0l-24-24a8,8,0,0,1,11.32-11.32L112,148.69l50.34-50.35A8,8,0,0,1,173.66,98.34ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,"warning-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24Zm0,192a88,88,0,1,1,88-88A88.1,88.1,0,0,1,128,216Zm-8-80V80a8,8,0,0,1,16,0v56a8,8,0,0,1-16,0Zm20,36a12,12,0,1,1-12-12A12,12,0,0,1,140,172Z"/></svg>`,"x-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M165.66,101.66,139.31,128l26.35,26.34a8,8,0,0,1-11.32,11.32L128,139.31l-26.34,26.35a8,8,0,0,1-11.32-11.32L116.69,128,90.34,101.66a8,8,0,0,1,11.32-11.32L128,116.69l26.34-26.35a8,8,0,0,1,11.32,11.32ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,pause:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M200,32H160a16,16,0,0,0-16,16V208a16,16,0,0,0,16,16h40a16,16,0,0,0,16-16V48A16,16,0,0,0,200,32Zm0,176H160V48h40ZM96,32H56A16,16,0,0,0,40,48V208a16,16,0,0,0,16,16H96a16,16,0,0,0,16-16V48A16,16,0,0,0,96,32Zm0,176H56V48H96Z"/></svg>`,play:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M232.4,114.49,88.32,26.35a16,16,0,0,0-16.2-.3A15.86,15.86,0,0,0,64,39.87V216.13A15.94,15.94,0,0,0,80,232a16.07,16.07,0,0,0,8.36-2.35L232.4,141.51a15.81,15.81,0,0,0,0-27ZM80,215.94V40l143.83,88Z"/></svg>`,"arrow-clockwise":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M240,56v48a8,8,0,0,1-8,8H184a8,8,0,0,1,0-16H211.4L184.81,71.64l-.25-.24a80,80,0,1,0-1.67,114.78,8,8,0,0,1,11,11.63A95.44,95.44,0,0,1,128,224h-1.32A96,96,0,1,1,195.75,60L224,85.8V56a8,8,0,1,1,16,0Z"/></svg>`,trash:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M216,48H176V40a24,24,0,0,0-24-24H104A24,24,0,0,0,80,40v8H40a8,8,0,0,0,0,16h8V208a16,16,0,0,0,16,16H192a16,16,0,0,0,16-16V64h8a8,8,0,0,0,0-16ZM96,40a8,8,0,0,1,8-8h48a8,8,0,0,1,8,8v8H96Zm96,168H64V64H192ZM112,104v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Zm48,0v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Z"/></svg>`,plus:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M224,128a8,8,0,0,1-8,8H136v80a8,8,0,0,1-16,0V136H40a8,8,0,0,1,0-16h80V40a8,8,0,0,1,16,0v80h80A8,8,0,0,1,224,128Z"/></svg>`,"lock-key":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,112a28,28,0,0,0-8,54.83V184a8,8,0,0,0,16,0V166.83A28,28,0,0,0,128,112Zm0,40a12,12,0,1,1,12-12A12,12,0,0,1,128,152Zm80-72H176V56a48,48,0,0,0-96,0V80H48A16,16,0,0,0,32,96V208a16,16,0,0,0,16,16H208a16,16,0,0,0,16-16V96A16,16,0,0,0,208,80ZM96,56a32,32,0,0,1,64,0V80H96ZM208,208H48V96H208V208Z"/></svg>`};function Q(e){return Cn((wn[e]??``).replace(`<svg `,`<svg aria-hidden="true" focusable="false" `))}var Tn=[{id:`overview`,label:`Overview`,icon:`gauge`},{id:`upstreams`,label:`Upstreams`,icon:`tree-structure`},{id:`certificates`,label:`Certificates`,icon:`certificate`},{id:`controls`,label:`Controls`,icon:`sliders-horizontal`},{id:`config`,label:`Config`,icon:`list-bullets`},{id:`logs`,label:`Logs`,icon:`terminal-window`}];function En(){let e=location.hash.replace(/^#\/?/,``);return Tn.some(t=>t.id===e)?e:`overview`}function Dn(e){let t=Math.floor(e/86400),n=Math.floor(e%86400/3600),r=Math.floor(e%3600/60);return t?`${t}d ${n}h`:n?`${n}h ${r}m`:`${r}m ${e%60}s`}var $=(e,t)=>x`<lit-material-status status=${e}>${t}</lit-material-status>`,On=e=>new Date(e).toLocaleTimeString(),kn=e=>/"status":5\d\d/.test(e)?`error`:/"status":4\d\d/.test(e)?`warning`:null,An=(e,t)=>[{key:`url`,label:`Backend`,mono:!0},{key:`weight`,label:`Weight`,numeric:!0},{key:`healthy`,label:`Health`,render:e=>e.healthy?$(`ok`,`Healthy`):$(`error`,`Failing checks`)},{key:`breaker`,label:`Breaker`,render:e=>e.breaker===null?$(`neutral`,`None`):e.breaker===`closed`?$(`ok`,`Closed`):e.breaker===`half_open`?$(`warning`,`Probing`):$(`error`,`Open`)},{key:`rotation`,label:`In rotation`,render:n=>x`<div class="row">
      <lit-material-switch aria-label="${n.url} in rotation" .checked=${!n.drained} ?disabled=${e.busy===`drain:${n.url}`}
        @change=${r=>e.drain(t,n.url,!r.target.checked)}></lit-material-switch>
      ${n.drained?$(`warning`,`Drained`):w}
    </div>`}],jn=[{key:`name`,label:`Name`,mono:!0},{key:`expires`,label:`Expires`},{key:`days_left`,label:`Days left`,numeric:!0,render:e=>e.days_left<7?$(`error`,String(e.days_left)):e.days_left<21?$(`warning`,String(e.days_left)):e.days_left},{key:`source`,label:`Source`,render:e=>e.source===`on_demand`?`On demand`:`Configured`}],Mn=class extends T{static properties={authed:{state:!0},view:{state:!0},loginError:{state:!0},data:{state:!0},error:{state:!0},samples:{state:!0},paused:{state:!0},filter:{state:!0},busy:{state:!0}};constructor(){super(),this.authed=!1,this.view=En(),this.data={},this.samples=[],this.paused=!1,this.filter=``,this.busy=``,this.lastTotal=null}createRenderRoot(){return this}connectedCallback(){super.connectedCallback(),this.onHash=()=>this.show(En()),addEventListener(`hashchange`,this.onHash),gn()&&this.check()}disconnectedCallback(){super.disconnectedCallback(),removeEventListener(`hashchange`,this.onHash),this.stop()}async check(){try{let e=await yn(`/status`);this.data={...this.data,status:e},this.authed=!0,this.loginError=``,this.show(this.view)}catch(e){this.authed=!1,this.loginError=e instanceof vn&&e.status===401?`That token isn't the admin token. Check RWS_ADMIN_TOKEN (or your token_env) on the server.`:`The admin API didn't answer: ${e.message}`,e instanceof vn&&e.status===401&&_n(``)}}login(e){e.preventDefault();let t=(this.querySelector(`#token`)?.value??``).trim();t?(_n(t),this.check()):this.loginError=`Paste the admin token.`}logout(){_n(``),this.stop(),this.authed=!1,this.data={}}async call(e,t){try{let n=await yn(e,t);return this.error=``,n}catch(e){throw e instanceof vn&&e.status===401?(this.logout(),this.loginError=`The server no longer accepts this token. Sign in again.`):this.error=e.message,e}}toast(e){this.querySelector(`#toast`)?.show(e)}show(e){this.stop(),this.view=e,location.hash!==`#/${e}`&&history.replaceState(null,``,`#/${e}`),this.error=``;let t={overview:2e3,upstreams:3e3,certificates:3e4,controls:5e3}[e];this.load(e),t&&(this.timer=setInterval(()=>this.load(e),t)),e===`logs`&&this.updateComplete.then(()=>this.follow()),queueMicrotask(()=>this.querySelector(`main`)?.focus({preventScroll:!0}))}stop(){clearInterval(this.timer),this.timer=void 0,this.abort?.abort(),this.abort=void 0}async load(e){let t=async(e,t)=>{try{let n=await this.call(t);this.data={...this.data,[e]:n}}catch{}};if(e===`overview`){await Promise.all([t(`status`,`/status`),t(`metrics`,`/metrics`)]);let e=this.data.metrics?.requests_total;if(typeof e==`number`){let t=Date.now();if(this.lastTotal){let n=Math.max(0,(e-this.lastTotal.total)/((t-this.lastTotal.at)/1e3));this.samples=[...this.samples,{x:t,y:Math.round(n*100)/100}].slice(-60)}this.lastTotal={total:e,at:t}}}else e===`upstreams`?await t(`upstreams`,`/upstreams`):e===`certificates`?await t(`certificates`,`/certificates`):e===`controls`?await Promise.all([t(`maintenance`,`/maintenance`),t(`blocklist`,`/blocklist`),t(`flags`,`/flags`),t(`rateLimit`,`/rate-limit`),this.loadCache()]):e===`config`&&await t(`config`,`/config`)}async loadCache(){try{let e=await yn(`/cache`);this.data={...this.data,cache:e}}catch(e){this.data={...this.data,cache:e instanceof vn&&e.status===404?null:this.data.cache}}}async follow(){this.abort=new AbortController;let e=this.querySelector(`lit-material-log-viewer`);if(e){e.highlight=kn;try{e.lines=await this.call(`/logs?n=200`),await bn(t=>e.appendLines(t),this.abort.signal)}catch(e){e.name!==`AbortError`&&!this.error&&(this.error=`The live log stopped: ${e.message}`)}}}async act(e,t,n,r){this.busy=e;try{let e=await this.call(t,n);r&&this.toast(r(e)),await this.load(this.view)}catch{}finally{this.busy=``}}drain(e,t,n){return this.act(`drain:${t}`,`/upstreams/${encodeURIComponent(e)}/drain`,{method:`PUT`,body:{backend:t,drained:n}},()=>n?`${t} drained: it gets no new requests`:`${t} is back in rotation`)}reload(){return this.act(`reload`,`/reload`,{method:`POST`},e=>e.applied?`Configuration reloaded`:`Reload kept the running configuration: ${e.errors.join(`; `)}`)}block(e){e.preventDefault();let t=this.querySelector(`#block-ip`),n=(t?.value??``).trim();n&&this.act(`block`,`/blocklist`,{method:`POST`,body:{ip:n}},e=>`${e.blocked} blocked`).then(()=>{!this.error&&t&&(t.value=``)})}render(){if(!this.authed)return this.renderLogin();let e=this.data.status,t=Tn.map(e=>({label:e.label,icon:Q(e.icon)}));return x`
      <div class="shell">
        <header class="top">
          <span class="brand"><span class="brand-mark" aria-hidden="true"></span>rws admin</span>
          ${e?e.ready?$(`ok`,`Ready`):$(`error`,`Not ready`):w}
          <span class="spacer"></span>
          ${e?x`<span class="meta">${e.version} · ${e.listeners?.main?.address??``}</span>`:w}
          <lit-material-button variant="text" @click=${()=>this.logout()}><span slot="icon" class="icon">${Q(`sign-out`)}</span>Sign out</lit-material-button>
        </header>
        <lit-material-adaptive-navigation class="nav" label="Sections" .destinations=${t}
          .selected=${Tn.findIndex(e=>e.id===this.view)}
          @change=${e=>this.show(Tn[e.target.selected].id)}></lit-material-adaptive-navigation>
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
          <p>Paste the admin token: the value of <code>RWS_ADMIN_TOKEN</code> (or the variable your <code>[admin] token_env</code> names). It stays in this tab only.</p>
          <form @submit=${e=>this.login(e)} novalidate>
            <lit-material-text-field id="token" variant="outlined" label="Admin token" type="password" autocomplete="current-password" required
              ?error=${!!this.loginError} error-text=${this.loginError??``}></lit-material-text-field>
            <lit-material-button type="submit">Sign in</lit-material-button>
          </form>
        </section>
      </div>
    `}renderView(){switch(this.view){case`upstreams`:return this.renderUpstreams();case`certificates`:return this.renderCertificates();case`controls`:return this.renderControls();case`config`:return this.renderConfig();case`logs`:return this.renderLogs();default:return this.renderOverview()}}skeleton(e=3){return x`<div class="grid" aria-busy="true">${Array.from({length:e},()=>x`<div class="tile"><lit-material-skeleton width="40%"></lit-material-skeleton><lit-material-skeleton variant="rounded" height="32px"></lit-material-skeleton></div>`)}</div>`}renderOverview(){let e=this.data.status,t=this.data.metrics;if(!e||!t)return x`<h1>Overview</h1>${this.skeleton(4)}`;let n=t.requests_total?(t.errors_total/t.requests_total*100).toFixed(2):`0.00`,r=this.samples.map(e=>e.y);return x`
      <h1>Overview</h1>
      <p class="lede">Up ${Dn(e.uptime_secs)}, process ${e.pid}. Counters refresh every 2 seconds.</p>
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
              unit="req/s" height="200" .formatX=${On} .series=${[{name:`Requests per second`,points:this.samples}]}></lit-material-line-chart>`}
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
            <lit-material-table caption="Backends of ${e.name}" caption-hidden .columns=${An(this,e.name)} .rows=${e.backends}></lit-material-table>
          </section>`)}
    `:x`<h1>Upstreams</h1>${this.skeleton(2)}`}renderCertificates(){let e=this.data.certificates;if(!e)return x`<h1>Certificates</h1>${this.skeleton(2)}`;let t=e.acme;return x`
      <h1>Certificates</h1>
      <p class="lede">Every certificate this server presents, soonest expiry first.</p>
      ${e.certificates.length===0?x`<lit-material-empty-state size="compact" headline="No TLS certificates">This server speaks plain HTTP. Set <code>tls_cert_file</code> and <code>tls_key_file</code>, or <code>[acme] domains</code>, to serve HTTPS.</lit-material-empty-state>`:x`<lit-material-table caption="Certificates" caption-hidden .columns=${jn} .rows=${e.certificates}></lit-material-table>`}
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
        <lit-material-stat label="Response cache" value=${e.cache?String(e.cache.size):`None`}
          caption=${e.cache?`${e.cache.hits} hits, ${e.cache.misses} misses`:`This server has no response cache.`}>
          ${e.cache?x`<lit-material-button slot="caption" variant="text" ?disabled=${this.busy===`cache`} @click=${()=>this.act(`cache`,`/cache`,{method:`DELETE`},()=>`Cache cleared`)}><span slot="icon" class="icon">${Q(`trash`)}</span>Clear</lit-material-button>`:w}
        </lit-material-stat>
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

      <section class="section" aria-labelledby="flags-title">
        <h2 id="flags-title">Feature flags</h2>
        ${e.flags.length===0?x`<lit-material-empty-state size="compact" headline="No feature flags">Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</lit-material-empty-state>`:x`<lit-material-table caption="Feature flags" caption-hidden density="compact" .columns=${[{key:`name`,label:`Flag`,mono:!0},{key:`enabled`,label:`Enabled`,render:e=>x`<lit-material-switch aria-label="${e.name}" .checked=${e.enabled}
          @change=${t=>this.act(`flag:${e.name}`,`/flags/${encodeURIComponent(e.name)}`,{method:`PUT`,body:{enabled:t.target.checked}},e=>`${e.name} ${e.enabled?`on`:`off`}`)}></lit-material-switch>`}]} .rows=${e.flags}></lit-material-table>`}
      </section>
    `}renderConfig(){let e=this.data.config;if(!e)return x`<h1>Config</h1>${this.skeleton(2)}`;let t=this.filter.toLowerCase(),n=Object.entries(e).filter(([e,n])=>!t||e.toLowerCase().includes(t)||n.toLowerCase().includes(t)).map(([e,t])=>({setting:e,value:t}));return x`
      <h1>Config</h1>
      <p class="lede">Every setting in effect. Secrets read (hidden). Change them in rws.config.toml or the environment, then reload from Controls.</p>
      <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${e=>this.filter=e.target.value}></lit-material-text-field>
      ${n.length===0?x`<lit-material-empty-state size="compact" headline="No setting matches “${this.filter}”">Clear the filter to see all ${Object.keys(e).length}.</lit-material-empty-state>`:x`<lit-material-table caption="Settings in effect" caption-hidden density="compact" .columns=${[{key:`setting`,label:`Setting`,mono:!0},{key:`value`,label:`Value`,mono:!0,render:e=>e.value===``?$(`neutral`,`empty`):e.value}]} .rows=${n}></lit-material-table>`}
    `}renderLogs(){return x`
      <h1>Logs</h1>
      <div class="row" style="margin-bottom: var(--space-200)">
        <p class="lede" style="margin: 0; flex: 1">The access log as it's written, newest at the bottom. Scroll up to read; it stops following until you're back at the bottom.</p>
        <lit-material-button variant="outlined" @click=${()=>this.paused=!this.paused}>
          <span slot="icon" class="icon">${Q(this.paused?`play`:`pause`)}</span>${this.paused?`Resume`:`Pause`}</lit-material-button>
      </div>
      <lit-material-log-viewer label="Access log" max-lines="1000" ?paused=${this.paused}></lit-material-log-viewer>
    `}};customElements.define(`rws-admin`,Mn);