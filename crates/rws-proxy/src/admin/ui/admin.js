var e=globalThis,t=e.ShadowRoot&&(e.ShadyCSS===void 0||e.ShadyCSS.nativeShadow)&&`adoptedStyleSheets`in Document.prototype&&`replace`in CSSStyleSheet.prototype,n=Symbol(),r=new WeakMap,i=class{constructor(e,t,r){if(this._$cssResult$=!0,r!==n)throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");this.cssText=e,this.t=t}get styleSheet(){let e=this.o,n=this.t;if(t&&e===void 0){let t=n!==void 0&&n.length===1;t&&(e=r.get(n)),e===void 0&&((this.o=e=new CSSStyleSheet).replaceSync(this.cssText),t&&r.set(n,e))}return e}toString(){return this.cssText}},a=e=>new i(typeof e==`string`?e:e+``,void 0,n),o=(e,...t)=>new i(e.length===1?e[0]:t.reduce((t,n,r)=>t+(e=>{if(!0===e._$cssResult$)return e.cssText;if(typeof e==`number`)return e;throw Error(`Value passed to 'css' function must be a 'css' function result: `+e+`. Use 'unsafeCSS' to pass non-literal values, but take care to ensure page security.`)})(n)+e[r+1],e[0]),e,n),s=(n,r)=>{if(t)n.adoptedStyleSheets=r.map(e=>e instanceof CSSStyleSheet?e:e.styleSheet);else for(let t of r){let r=document.createElement(`style`),i=e.litNonce;i!==void 0&&r.setAttribute(`nonce`,i),r.textContent=t.cssText,n.appendChild(r)}},c=t?e=>e:e=>e instanceof CSSStyleSheet?(e=>{let t=``;for(let n of e.cssRules)t+=n.cssText;return a(t)})(e):e,{is:l,defineProperty:u,getOwnPropertyDescriptor:d,getOwnPropertyNames:f,getOwnPropertySymbols:p,getPrototypeOf:m}=Object,h=globalThis,ee=h.trustedTypes,te=ee?ee.emptyScript:``,ne=h.reactiveElementPolyfillSupport,re=(e,t)=>e,g={toAttribute(e,t){switch(t){case Boolean:e=e?te:null;break;case Object:case Array:e=e==null?e:JSON.stringify(e)}return e},fromAttribute(e,t){let n=e;switch(t){case Boolean:n=e!==null;break;case Number:n=e===null?null:Number(e);break;case Object:case Array:try{n=JSON.parse(e)}catch{n=null}}return n}},_=(e,t)=>!l(e,t),ie={attribute:!0,type:String,converter:g,reflect:!1,useDefault:!1,hasChanged:_};Symbol.metadata??=Symbol(`metadata`),h.litPropertyMetadata??=new WeakMap;var ae=class extends HTMLElement{static addInitializer(e){this._$Ei(),(this.l??=[]).push(e)}static get observedAttributes(){return this.finalize(),this._$Eh&&[...this._$Eh.keys()]}static createProperty(e,t=ie){if(t.state&&(t.attribute=!1),this._$Ei(),this.prototype.hasOwnProperty(e)&&((t=Object.create(t)).wrapped=!0),this.elementProperties.set(e,t),!t.noAccessor){let n=Symbol(),r=this.getPropertyDescriptor(e,n,t);r!==void 0&&u(this.prototype,e,r)}}static getPropertyDescriptor(e,t,n){let{get:r,set:i}=d(this.prototype,e)??{get(){return this[t]},set(e){this[t]=e}};return{get:r,set(t){let a=r?.call(this);i?.call(this,t),this.requestUpdate(e,a,n)},configurable:!0,enumerable:!0}}static getPropertyOptions(e){return this.elementProperties.get(e)??ie}static _$Ei(){if(this.hasOwnProperty(re(`elementProperties`)))return;let e=m(this);e.finalize(),e.l!==void 0&&(this.l=[...e.l]),this.elementProperties=new Map(e.elementProperties)}static finalize(){if(this.hasOwnProperty(re(`finalized`)))return;if(this.finalized=!0,this._$Ei(),this.hasOwnProperty(re(`properties`))){let e=this.properties,t=[...f(e),...p(e)];for(let n of t)this.createProperty(n,e[n])}let e=this[Symbol.metadata];if(e!==null){let t=litPropertyMetadata.get(e);if(t!==void 0)for(let[e,n]of t)this.elementProperties.set(e,n)}this._$Eh=new Map;for(let[e,t]of this.elementProperties){let n=this._$Eu(e,t);n!==void 0&&this._$Eh.set(n,e)}this.elementStyles=this.finalizeStyles(this.styles)}static finalizeStyles(e){let t=[];if(Array.isArray(e)){let n=new Set(e.flat(1/0).reverse());for(let e of n)t.unshift(c(e))}else e!==void 0&&t.push(c(e));return t}static _$Eu(e,t){let n=t.attribute;return!1===n?void 0:typeof n==`string`?n:typeof e==`string`?e.toLowerCase():void 0}constructor(){super(),this._$Ep=void 0,this.isUpdatePending=!1,this.hasUpdated=!1,this._$Em=null,this._$Ev()}_$Ev(){this._$ES=new Promise(e=>this.enableUpdating=e),this._$AL=new Map,this._$E_(),this.requestUpdate(),this.constructor.l?.forEach(e=>e(this))}addController(e){(this._$EO??=new Set).add(e),this.renderRoot!==void 0&&this.isConnected&&e.hostConnected?.()}removeController(e){this._$EO?.delete(e)}_$E_(){let e=new Map,t=this.constructor.elementProperties;for(let n of t.keys())this.hasOwnProperty(n)&&(e.set(n,this[n]),delete this[n]);e.size>0&&(this._$Ep=e)}createRenderRoot(){let e=this.shadowRoot??this.attachShadow(this.constructor.shadowRootOptions);return s(e,this.constructor.elementStyles),e}connectedCallback(){this.renderRoot??=this.createRenderRoot(),this.enableUpdating(!0),this._$EO?.forEach(e=>e.hostConnected?.())}enableUpdating(e){}disconnectedCallback(){this._$EO?.forEach(e=>e.hostDisconnected?.())}attributeChangedCallback(e,t,n){this._$AK(e,n)}_$ET(e,t){let n=this.constructor.elementProperties.get(e),r=this.constructor._$Eu(e,n);if(r!==void 0&&!0===n.reflect){let i=(n.converter?.toAttribute===void 0?g:n.converter).toAttribute(t,n.type);this._$Em=e,i==null?this.removeAttribute(r):this.setAttribute(r,i),this._$Em=null}}_$AK(e,t){let n=this.constructor,r=n._$Eh.get(e);if(r!==void 0&&this._$Em!==r){let e=n.getPropertyOptions(r),i=typeof e.converter==`function`?{fromAttribute:e.converter}:e.converter?.fromAttribute===void 0?g:e.converter;this._$Em=r;let a=i.fromAttribute(t,e.type);this[r]=a??this._$Ej?.get(r)??a,this._$Em=null}}requestUpdate(e,t,n,r=!1,i){if(e!==void 0){let a=this.constructor;if(!1===r&&(i=this[e]),n??=a.getPropertyOptions(e),!((n.hasChanged??_)(i,t)||n.useDefault&&n.reflect&&i===this._$Ej?.get(e)&&!this.hasAttribute(a._$Eu(e,n))))return;this.C(e,t,n)}!1===this.isUpdatePending&&(this._$ES=this._$EP())}C(e,t,{useDefault:n,reflect:r,wrapped:i},a){n&&!(this._$Ej??=new Map).has(e)&&(this._$Ej.set(e,a??t??this[e]),!0!==i||a!==void 0)||(this._$AL.has(e)||(this.hasUpdated||n||(t=void 0),this._$AL.set(e,t)),!0===r&&this._$Em!==e&&(this._$Eq??=new Set).add(e))}async _$EP(){this.isUpdatePending=!0;try{await this._$ES}catch(e){Promise.reject(e)}let e=this.scheduleUpdate();return e!=null&&await e,!this.isUpdatePending}scheduleUpdate(){return this.performUpdate()}performUpdate(){if(!this.isUpdatePending)return;if(!this.hasUpdated){if(this.renderRoot??=this.createRenderRoot(),this._$Ep){for(let[e,t]of this._$Ep)this[e]=t;this._$Ep=void 0}let e=this.constructor.elementProperties;if(e.size>0)for(let[t,n]of e){let{wrapped:e}=n,r=this[t];!0!==e||this._$AL.has(t)||r===void 0||this.C(t,void 0,n,r)}}let e=!1,t=this._$AL;try{e=this.shouldUpdate(t),e?(this.willUpdate(t),this._$EO?.forEach(e=>e.hostUpdate?.()),this.update(t)):this._$EM()}catch(t){throw e=!1,this._$EM(),t}e&&this._$AE(t)}willUpdate(e){}_$AE(e){this._$EO?.forEach(e=>e.hostUpdated?.()),this.hasUpdated||(this.hasUpdated=!0,this.firstUpdated(e)),this.updated(e)}_$EM(){this._$AL=new Map,this.isUpdatePending=!1}get updateComplete(){return this.getUpdateComplete()}getUpdateComplete(){return this._$ES}shouldUpdate(e){return!0}update(e){this._$Eq&&=this._$Eq.forEach(e=>this._$ET(e,this[e])),this._$EM()}updated(e){}firstUpdated(e){}};ae.elementStyles=[],ae.shadowRootOptions={mode:`open`},ae[re(`elementProperties`)]=new Map,ae[re(`finalized`)]=new Map,ne?.({ReactiveElement:ae}),(h.reactiveElementVersions??=[]).push(`2.1.2`);var oe=globalThis,v=e=>e,se=oe.trustedTypes,ce=se?se.createPolicy(`lit-html`,{createHTML:e=>e}):void 0,le=`$lit$`,y=`lit$${Math.random().toFixed(9).slice(2)}$`,ue=`?`+y,de=`<${ue}>`,fe=document,pe=()=>fe.createComment(``),me=e=>e===null||typeof e!=`object`&&typeof e!=`function`,he=Array.isArray,ge=e=>he(e)||typeof e?.[Symbol.iterator]==`function`,_e=`[ 	
\f\r]`,ve=/<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g,ye=/-->/g,be=/>/g,xe=RegExp(`>|${_e}(?:([^\\s"'>=/]+)(${_e}*=${_e}*(?:[^ \t\n\f\r"'\`<>=]|("|')|))|$)`,`g`),Se=/'/g,Ce=/"/g,we=/^(?:script|style|textarea|title)$/i,Te=e=>(t,...n)=>({_$litType$:e,strings:t,values:n}),b=Te(1),x=Te(2),Ee=Symbol.for(`lit-noChange`),S=Symbol.for(`lit-nothing`),De=new WeakMap,Oe=fe.createTreeWalker(fe,129);function ke(e,t){if(!he(e)||!e.hasOwnProperty(`raw`))throw Error(`invalid template strings array`);return ce===void 0?t:ce.createHTML(t)}var Ae=(e,t)=>{let n=e.length-1,r=[],i,a=t===2?`<svg>`:t===3?`<math>`:``,o=ve;for(let t=0;t<n;t++){let n=e[t],s,c,l=-1,u=0;for(;u<n.length&&(o.lastIndex=u,c=o.exec(n),c!==null);)u=o.lastIndex,o===ve?c[1]===`!--`?o=ye:c[1]===void 0?c[2]===void 0?c[3]!==void 0&&(o=xe):(we.test(c[2])&&(i=RegExp(`</`+c[2],`g`)),o=xe):o=be:o===xe?c[0]===`>`?(o=i??ve,l=-1):c[1]===void 0?l=-2:(l=o.lastIndex-c[2].length,s=c[1],o=c[3]===void 0?xe:c[3]===`"`?Ce:Se):o===Ce||o===Se?o=xe:o===ye||o===be?o=ve:(o=xe,i=void 0);let d=o===xe&&e[t+1].startsWith(`/>`)?` `:``;a+=o===ve?n+de:l>=0?(r.push(s),n.slice(0,l)+le+n.slice(l)+y+d):n+y+(l===-2?t:d)}return[ke(e,a+(e[n]||`<?>`)+(t===2?`</svg>`:t===3?`</math>`:``)),r]},je=class e{constructor({strings:t,_$litType$:n},r){let i;this.parts=[];let a=0,o=0,s=t.length-1,c=this.parts,[l,u]=Ae(t,n);if(this.el=e.createElement(l,r),Oe.currentNode=this.el.content,n===2||n===3){let e=this.el.content.firstChild;e.replaceWith(...e.childNodes)}for(;(i=Oe.nextNode())!==null&&c.length<s;){if(i.nodeType===1){if(i.hasAttributes())for(let e of i.getAttributeNames())if(e.endsWith(le)){let t=u[o++],n=i.getAttribute(e).split(y),r=/([.?@])?(.*)/.exec(t);c.push({type:1,index:a,name:r[2],strings:n,ctor:r[1]===`.`?Ie:r[1]===`?`?Le:r[1]===`@`?Re:Fe}),i.removeAttribute(e)}else e.startsWith(y)&&(c.push({type:6,index:a}),i.removeAttribute(e));if(we.test(i.tagName)){let e=i.textContent.split(y),t=e.length-1;if(t>0){i.textContent=se?se.emptyScript:``;for(let n=0;n<t;n++)i.append(e[n],pe()),Oe.nextNode(),c.push({type:2,index:++a});i.append(e[t],pe())}}}else if(i.nodeType===8){if(i.data===ue)c.push({type:2,index:a});else{let e=-1;for(;(e=i.data.indexOf(y,e+1))!==-1;)c.push({type:7,index:a}),e+=y.length-1}}a++}}static createElement(e,t){let n=fe.createElement(`template`);return n.innerHTML=e,n}};function Me(e,t,n=e,r){if(t===Ee)return t;let i=r===void 0?n._$Cl:n._$Co?.[r],a=me(t)?void 0:t._$litDirective$;return i?.constructor!==a&&(i?._$AO?.(!1),a===void 0?i=void 0:(i=new a(e),i._$AT(e,n,r)),r===void 0?n._$Cl=i:(n._$Co??=[])[r]=i),i!==void 0&&(t=Me(e,i._$AS(e,t.values),i,r)),t}var Ne=class{constructor(e,t){this._$AV=[],this._$AN=void 0,this._$AD=e,this._$AM=t}get parentNode(){return this._$AM.parentNode}get _$AU(){return this._$AM._$AU}u(e){let{el:{content:t},parts:n}=this._$AD,r=(e?.creationScope??fe).importNode(t,!0);Oe.currentNode=r;let i=Oe.nextNode(),a=0,o=0,s=n[0];for(;s!==void 0;){if(a===s.index){let t;s.type===2?t=new Pe(i,i.nextSibling,this,e):s.type===1?t=new s.ctor(i,s.name,s.strings,this,e):s.type===6&&(t=new ze(i,this,e)),this._$AV.push(t),s=n[++o]}a!==s?.index&&(i=Oe.nextNode(),a++)}return Oe.currentNode=fe,r}p(e){let t=0;for(let n of this._$AV)n!==void 0&&(n.strings===void 0?n._$AI(e[t]):(n._$AI(e,n,t),t+=n.strings.length-2)),t++}},Pe=class e{get _$AU(){return this._$AM?._$AU??this._$Cv}constructor(e,t,n,r){this.type=2,this._$AH=S,this._$AN=void 0,this._$AA=e,this._$AB=t,this._$AM=n,this.options=r,this._$Cv=r?.isConnected??!0}get parentNode(){let e=this._$AA.parentNode,t=this._$AM;return t!==void 0&&e?.nodeType===11&&(e=t.parentNode),e}get startNode(){return this._$AA}get endNode(){return this._$AB}_$AI(e,t=this){e=Me(this,e,t),me(e)?e===S||e==null||e===``?(this._$AH!==S&&this._$AR(),this._$AH=S):e!==this._$AH&&e!==Ee&&this._(e):e._$litType$===void 0?e.nodeType===void 0?ge(e)?this.k(e):this._(e):this.T(e):this.$(e)}O(e){return this._$AA.parentNode.insertBefore(e,this._$AB)}T(e){this._$AH!==e&&(this._$AR(),this._$AH=this.O(e))}_(e){this._$AH!==S&&me(this._$AH)?this._$AA.nextSibling.data=e:this.T(fe.createTextNode(e)),this._$AH=e}$(e){let{values:t,_$litType$:n}=e,r=typeof n==`number`?this._$AC(e):(n.el===void 0&&(n.el=je.createElement(ke(n.h,n.h[0]),this.options)),n);if(this._$AH?._$AD===r)this._$AH.p(t);else{let e=new Ne(r,this),n=e.u(this.options);e.p(t),this.T(n),this._$AH=e}}_$AC(e){let t=De.get(e.strings);return t===void 0&&De.set(e.strings,t=new je(e)),t}k(t){he(this._$AH)||(this._$AH=[],this._$AR());let n=this._$AH,r,i=0;for(let a of t)i===n.length?n.push(r=new e(this.O(pe()),this.O(pe()),this,this.options)):r=n[i],r._$AI(a),i++;i<n.length&&(this._$AR(r&&r._$AB.nextSibling,i),n.length=i)}_$AR(e=this._$AA.nextSibling,t){for(this._$AP?.(!1,!0,t);e!==this._$AB;){let t=v(e).nextSibling;v(e).remove(),e=t}}setConnected(e){this._$AM===void 0&&(this._$Cv=e,this._$AP?.(e))}},Fe=class{get tagName(){return this.element.tagName}get _$AU(){return this._$AM._$AU}constructor(e,t,n,r,i){this.type=1,this._$AH=S,this._$AN=void 0,this.element=e,this.name=t,this._$AM=r,this.options=i,n.length>2||n[0]!==``||n[1]!==``?(this._$AH=Array(n.length-1).fill(new String),this.strings=n):this._$AH=S}_$AI(e,t=this,n,r){let i=this.strings,a=!1;if(i===void 0)e=Me(this,e,t,0),a=!me(e)||e!==this._$AH&&e!==Ee,a&&(this._$AH=e);else{let r=e,o,s;for(e=i[0],o=0;o<i.length-1;o++)s=Me(this,r[n+o],t,o),s===Ee&&(s=this._$AH[o]),a||=!me(s)||s!==this._$AH[o],s===S?e=S:e!==S&&(e+=(s??``)+i[o+1]),this._$AH[o]=s}a&&!r&&this.j(e)}j(e){e===S?this.element.removeAttribute(this.name):this.element.setAttribute(this.name,e??``)}},Ie=class extends Fe{constructor(){super(...arguments),this.type=3}j(e){this.element[this.name]=e===S?void 0:e}},Le=class extends Fe{constructor(){super(...arguments),this.type=4}j(e){this.element.toggleAttribute(this.name,!!e&&e!==S)}},Re=class extends Fe{constructor(e,t,n,r,i){super(e,t,n,r,i),this.type=5}_$AI(e,t=this){if((e=Me(this,e,t,0)??S)===Ee)return;let n=this._$AH,r=e===S&&n!==S||e.capture!==n.capture||e.once!==n.once||e.passive!==n.passive,i=e!==S&&(n===S||r);r&&this.element.removeEventListener(this.name,this,n),i&&this.element.addEventListener(this.name,this,e),this._$AH=e}handleEvent(e){typeof this._$AH==`function`?this._$AH.call(this.options?.host??this.element,e):this._$AH.handleEvent(e)}},ze=class{constructor(e,t,n){this.element=e,this.type=6,this._$AN=void 0,this._$AM=t,this.options=n}get _$AU(){return this._$AM._$AU}_$AI(e){Me(this,e)}},Be={M:le,P:y,A:ue,C:1,L:Ae,R:Ne,D:ge,V:Me,I:Pe,H:Fe,N:Le,U:Re,B:Ie,F:ze},Ve=oe.litHtmlPolyfillSupport;Ve?.(je,Pe),(oe.litHtmlVersions??=[]).push(`3.3.3`);var He=(e,t,n)=>{let r=n?.renderBefore??t,i=r._$litPart$;if(i===void 0){let e=n?.renderBefore??null;r._$litPart$=i=new Pe(t.insertBefore(pe(),e),e,void 0,n??{})}return i._$AI(e),i},Ue=globalThis,C=class extends ae{constructor(){super(...arguments),this.renderOptions={host:this},this._$Do=void 0}createRenderRoot(){let e=super.createRenderRoot();return this.renderOptions.renderBefore??=e.firstChild,e}update(e){let t=this.render();this.hasUpdated||(this.renderOptions.isConnected=this.isConnected),super.update(e),this._$Do=He(t,this.renderRoot,this.renderOptions)}connectedCallback(){super.connectedCallback(),this._$Do?.setConnected(!0)}disconnectedCallback(){super.disconnectedCallback(),this._$Do?.setConnected(!1)}render(){return Ee}};C._$litElement$=!0,C.finalized=!0,Ue.litElementHydrateSupport?.({LitElement:C});var We=Ue.litElementPolyfillSupport;We?.({LitElement:C}),(Ue.litElementVersions??=[]).push(`4.2.2`);var w=e=>(t,n)=>{n===void 0?customElements.define(e,t):n.addInitializer(()=>{customElements.define(e,t)})},Ge={attribute:!0,type:String,converter:g,reflect:!1,hasChanged:_},Ke=(e=Ge,t,n)=>{let{kind:r,metadata:i}=n,a=globalThis.litPropertyMetadata.get(i);if(a===void 0&&globalThis.litPropertyMetadata.set(i,a=new Map),r===`setter`&&((e=Object.create(e)).wrapped=!0),a.set(n.name,e),r===`accessor`){let{name:r}=n;return{set(n){let i=t.get.call(this);t.set.call(this,n),this.requestUpdate(r,i,e,!0,n)},init(t){return t!==void 0&&this.C(r,void 0,e,t),t}}}if(r===`setter`){let{name:r}=n;return function(n){let i=this[r];t.call(this,n),this.requestUpdate(r,i,e,!0,n)}}throw Error(`Unsupported decorator location: `+r)};function T(e){return(t,n)=>typeof n==`object`?Ke(e,t,n):((e,t,n)=>{let r=t.hasOwnProperty(n);return t.constructor.createProperty(n,e),r?Object.getOwnPropertyDescriptor(t,n):void 0})(e,t,n)}function E(e){return T({...e,state:!0,attribute:!1})}var qe=(e,t,n)=>(n.configurable=!0,n.enumerable=!0,Reflect.decorate&&typeof t!=`object`&&Object.defineProperty(e,t,n),n);function D(e,t){return(n,r,i)=>{let a=t=>t.renderRoot?.querySelector(e)??null;if(t){let{get:e,set:t}=typeof r==`object`?n:i??(()=>{let e=Symbol();return{get(){return this[e]},set(t){this[e]=t}}})();return qe(n,r,{get(){let n=e.call(this);return n===void 0&&(n=a(this),(n!==null||this.hasUpdated)&&t.call(this,n)),n}})}return qe(n,r,{get(){return a(this)}})}}var Je=o`
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
`,Ye=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Xe={info:x`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <line x1="12" y1="11" x2="12" y2="16" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="7.5" r="1.25" fill="currentColor"></circle>
    </svg>
  `,success:x`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <polyline points="7,12.5 10.5,16 17,8.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></polyline>
    </svg>
  `,warning:x`
    <svg viewBox="0 0 24 24">
      <polygon points="12,3 22,20 2,20" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"></polygon>
      <line x1="12" y1="9" x2="12" y2="14" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="17" r="1.15" fill="currentColor"></circle>
    </svg>
  `,error:x`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <line x1="12" y1="7" x2="12" y2="13" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="16.5" r="1.25" fill="currentColor"></circle>
    </svg>
  `},Ze=class extends C{constructor(){super(...arguments),this.variant=`info`,this.dismissible=!1,this.roleIsOwn=!1}static{this.styles=Je}willUpdate(){(!this.hasAttribute(`role`)||this.roleIsOwn)&&(this.setAttribute(`role`,this.variant===`warning`||this.variant===`error`?`alert`:`status`),this.roleIsOwn=!0)}dismiss(){this.hidden=!0,this.dispatchEvent(new Event(`close`,{bubbles:!0}))}render(){return b`
      <div class="alert" part="alert">
        <span class="icon" part="icon" aria-hidden="true">
          <slot name="icon">${Xe[this.variant]}</slot>
        </span>
        <div class="body" part="body">
          <slot name="title" class="title"></slot>
          <div class="description" part="description"><slot></slot></div>
          <slot name="action" class="actions"></slot>
        </div>
        ${this.dismissible?b`
              <button class="close" part="close" type="button" aria-label="Dismiss" @click=${this.dismiss}>
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
                  <line x1="6" y1="18" x2="18" y2="6" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
                </svg>
              </button>
            `:S}
      </div>
    `}};Ye([T()],Ze.prototype,`variant`,void 0),Ye([T({type:Boolean,reflect:!0})],Ze.prototype,`dismissible`,void 0),Ze=Ye([w(`lit-material-alert`)],Ze);var Qe=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},$e=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}},et=o`
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
`,O=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},tt=0,k=class extends C{static{this.styles=et}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`button`,this.disabled=!1,this.name=``,this.value=``,this.href=``,this.target=``,this.ripple=new Qe(this),this.focusRing=new $e(this),this.labelId=`lit-material-button-label-${++tt}`,this.internals=this.attachInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}render(){let e=b`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      <slot name="icon" class="icon"></slot>
      <span class="label" id=${this.labelId}><slot></slot></span>
    `;return this.href?b`
        <a
          class="button"
          part="button"
          href=${this.disabled?S:this.href}
          target=${this.target||S}
          rel=${this.target===`_blank`?`noopener noreferrer`:S}
          aria-disabled=${this.disabled?`true`:S}
          aria-labelledby=${this.labelId}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${e}
        </a>
      `:b`
      <button
        class="button"
        part="button"
        type="button"
        ?disabled=${this.disabled}
        name=${this.name||S}
        value=${this.value||S}
        aria-labelledby=${this.labelId}
        @click=${this.handleClick}
      >
        ${e}
      </button>
    `}handleClick(e){this.disabled?(e.preventDefault(),e.stopImmediatePropagation()):this.type===`submit`?this.internals.form?.requestSubmit():this.type===`reset`&&this.internals.form?.reset()}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};O([T()],k.prototype,`variant`,void 0),O([T()],k.prototype,`type`,void 0),O([T({type:Boolean,reflect:!0})],k.prototype,`disabled`,void 0),O([T()],k.prototype,`name`,void 0),O([T()],k.prototype,`value`,void 0),O([T({reflect:!0})],k.prototype,`form`,void 0),O([T()],k.prototype,`href`,void 0),O([T()],k.prototype,`target`,void 0),O([D(`.state-layer`)],k.prototype,`stateLayerElement`,void 0),k=O([w(`lit-material-button`)],k);var nt=o`
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
`,rt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},A=class extends C{static{this.styles=nt}constructor(){super(),this.variant=`standard`,this.position=`start`,this.selected=-1,this.open=!1,this.disableBackdropClose=!1,this.reopenOnClose=!1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected(),e.has(`open`)&&this.variant===`modal`&&this.dialogElement&&(this.open&&!this.dialogElement.open?this.dialogElement.showModal():!this.open&&this.dialogElement.open&&this.dialogElement.close())}show(){this.open=!0}close(){this.open=!1}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-drawer-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}renderNav(){let e=this.variant===`modal`;return b`
      <nav
        class="drawer ${this.variant}"
        part="drawer"
        tabindex=${e?`-1`:S}
        ?autofocus=${e}
      >
        <div class="header" part="header"><slot name="header"></slot></div>
        <div class="content" part="content"><slot @slotchange=${this.handleSlotChange}></slot></div>
      </nav>
    `}render(){return this.variant===`modal`?b`
      <dialog
        class="dialog"
        part="dialog"
        @close=${this.handleClose}
        @cancel=${this.handleCancel}
        @click=${this.handleBackdropClick}
      >
        ${this.renderNav()}
      </dialog>
    `:this.renderNav()}handleClose(){this.reopenOnClose?(this.reopenOnClose=!1,this.dialogElement?.showModal()):(this.open=!1,this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0})))}handleCancel(e){this.dispatchEvent(new Event(`cancel`,{bubbles:!0,cancelable:!0,composed:!0}))||(e.preventDefault(),this.reopenOnClose=!0)}handleBackdropClick(e){!this.disableBackdropClose&&e.target===this.dialogElement&&this.dialogElement?.close()}};rt([T()],A.prototype,`variant`,void 0),rt([T({reflect:!0})],A.prototype,`position`,void 0),rt([T({type:Number})],A.prototype,`selected`,void 0),rt([T({type:Boolean,reflect:!0})],A.prototype,`open`,void 0),rt([T({type:Boolean,attribute:`disable-backdrop-close`})],A.prototype,`disableBackdropClose`,void 0),rt([D(`dialog`)],A.prototype,`dialogElement`,void 0),A=rt([w(`lit-material-navigation-drawer`)],A);var it=o`
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
`,at=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ot=class extends C{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Qe(this),this.focusRing=new $e(this)}static{this.styles=it}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return b`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      <slot name="icon" class="icon"></slot>
      <span class="label"><slot></slot></span>
      <slot name="badge" class="badge"></slot>
    `}render(){let e=this.selected?`page`:S;return this.href?b`
        <a
          class="item"
          part="item"
          href=${this.disabled?S:this.href}
          target=${this.target||S}
          rel=${this.target===`_blank`?`noopener noreferrer`:S}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:S}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:b`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};at([T({type:Boolean,reflect:!0})],ot.prototype,`selected`,void 0),at([T({type:Boolean,reflect:!0})],ot.prototype,`disabled`,void 0),at([T()],ot.prototype,`href`,void 0),at([T()],ot.prototype,`target`,void 0),at([D(`.state-layer`)],ot.prototype,`stateLayerElement`,void 0),ot=at([w(`lit-material-navigation-drawer-item`)],ot);var st=o`
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
`,ct=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},lt=class extends C{static{this.styles=st}constructor(){super(),this.alignment=`top`,this.selected=-1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected()}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-rail-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}render(){return b`
      <div class="rail" part="rail">
        <div class="fab" part="fab"><slot name="fab"></slot></div>
        <div class="items ${this.alignment}" part="items">
          <slot @slotchange=${this.handleSlotChange}></slot>
        </div>
      </div>
    `}};ct([T()],lt.prototype,`alignment`,void 0),ct([T({type:Number})],lt.prototype,`selected`,void 0),lt=ct([w(`lit-material-navigation-rail`)],lt);var ut=o`
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
`,dt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ft=class extends C{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Qe(this),this.focusRing=new $e(this)}static{this.styles=ut}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return b`
      <span class="icon-container" part="icon-container">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="state-layer" part="ripple"></div>
        <slot name="icon" class="icon"></slot>
        <slot name="badge" class="badge"></slot>
      </span>
      <span class="label"><slot></slot></span>
    `}render(){let e=this.selected?`page`:S;return this.href?b`
        <a
          class="item"
          part="item"
          href=${this.disabled?S:this.href}
          target=${this.target||S}
          rel=${this.target===`_blank`?`noopener noreferrer`:S}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:S}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:b`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};dt([T({type:Boolean,reflect:!0})],ft.prototype,`selected`,void 0),dt([T({type:Boolean,reflect:!0})],ft.prototype,`disabled`,void 0),dt([T()],ft.prototype,`href`,void 0),dt([T()],ft.prototype,`target`,void 0),dt([D(`.state-layer`)],ft.prototype,`stateLayerElement`,void 0),ft=dt([w(`lit-material-navigation-rail-item`)],ft);var pt=o`
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
`,mt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ht=class extends C{static{this.styles=pt}constructor(){super(),this.selected=-1,this.handleSlotChange=()=>this.syncSelected(),this.handleClick=e=>{let t=e.composedPath(),n=this.items,r=n.find(e=>t.includes(e));if(!r||r.disabled)return;let i=n.indexOf(r),a=i!==this.selected;this.selected=i,a&&this.dispatchEvent(new Event(`change`,{bubbles:!0}))},this.addEventListener(`click`,this.handleClick)}connectedCallback(){super.connectedCallback(),this.syncSelected()}updated(e){e.has(`selected`)&&this.syncSelected()}get items(){return typeof this.querySelectorAll==`function`?Array.from(this.querySelectorAll(`lit-material-navigation-bar-item`)):[]}syncSelected(){this.items.forEach((e,t)=>{e.selected=t===this.selected})}render(){return b`
      <div class="bar" part="bar">
        <slot @slotchange=${this.handleSlotChange}></slot>
      </div>
    `}};mt([T({type:Number})],ht.prototype,`selected`,void 0),ht=mt([w(`lit-material-navigation-bar`)],ht);var gt=o`
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
`,_t=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},vt=class extends C{constructor(){super(...arguments),this.selected=!1,this.disabled=!1,this.href=``,this.target=``,this.ripple=new Qe(this),this.focusRing=new $e(this)}static{this.styles=gt}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}updated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}get content(){return b`
      <span class="icon-container" part="icon-container">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="state-layer" part="ripple"></div>
        <slot name="icon" class="icon"></slot>
        <slot name="badge" class="badge"></slot>
      </span>
      <span class="label"><slot></slot></span>
    `}render(){let e=this.selected?`page`:S;return this.href?b`
        <a
          class="item"
          part="item"
          href=${this.disabled?S:this.href}
          target=${this.target||S}
          rel=${this.target===`_blank`?`noopener noreferrer`:S}
          aria-current=${e}
          aria-disabled=${this.disabled?`true`:S}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${this.content}
        </a>
      `:b`
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
    `}handleClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};_t([T({type:Boolean,reflect:!0})],vt.prototype,`selected`,void 0),_t([T({type:Boolean,reflect:!0})],vt.prototype,`disabled`,void 0),_t([T()],vt.prototype,`href`,void 0),_t([T()],vt.prototype,`target`,void 0),_t([D(`.state-layer`)],vt.prototype,`stateLayerElement`,void 0),vt=_t([w(`lit-material-navigation-bar-item`)],vt);var yt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},bt=class extends C{constructor(){super(...arguments),this.destinations=[],this.selected=-1,this.expanded=`drawer`,this.label=`Main`}static{this.styles=o`
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
  `}onChange(e){e.stopPropagation();let t=e.target.selected;t!==this.selected&&(this.selected=t,this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})))}icon(e){return e.icon===void 0?S:b`<span slot="icon" aria-hidden="true">${e.icon}</span>`}render(){let e=this.destinations;return b`
      <nav aria-label=${this.label}>
        <div class="drawer">
          <lit-material-navigation-drawer .selected=${this.selected} @change=${this.onChange}>
            ${e.map((e,t)=>b`<lit-material-navigation-drawer-item ?selected=${t===this.selected} ?disabled=${!!e.disabled} href=${e.href??S}
              >${this.icon(e)}${e.label}</lit-material-navigation-drawer-item>`)}
          </lit-material-navigation-drawer>
        </div>
        <div class="rail">
          <lit-material-navigation-rail .selected=${this.selected} @change=${this.onChange}>
            ${e.map((e,t)=>b`<lit-material-navigation-rail-item ?selected=${t===this.selected} ?disabled=${!!e.disabled} href=${e.href??S}
              >${this.icon(e)}${e.label}</lit-material-navigation-rail-item>`)}
          </lit-material-navigation-rail>
        </div>
        <div class="bar">
          <lit-material-navigation-bar .selected=${this.selected} @change=${this.onChange}>
            ${e.map((e,t)=>b`<lit-material-navigation-bar-item ?selected=${t===this.selected} ?disabled=${!!e.disabled} href=${e.href??S}
              >${this.icon(e)}${e.label}</lit-material-navigation-bar-item>`)}
          </lit-material-navigation-bar>
        </div>
      </nav>
    `}};yt([T({attribute:!1})],bt.prototype,`destinations`,void 0),yt([T({type:Number})],bt.prototype,`selected`,void 0),yt([T({reflect:!0})],bt.prototype,`expanded`,void 0),yt([T()],bt.prototype,`label`,void 0),bt=yt([w(`lit-material-adaptive-navigation`)],bt);var xt=o`
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
`,St=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Ct={ok:x`<circle cx="10" cy="10" r="8"/><path d="M6.5 10.2l2.4 2.4 4.6-4.8"/>`,warning:x`<path d="M10 3l7.5 13h-15z"/><path d="M10 8.2v3.6M10 14.2v.1"/>`,error:x`<circle cx="10" cy="10" r="8"/><path d="M7.2 7.2l5.6 5.6M12.8 7.2l-5.6 5.6"/>`,info:x`<circle cx="10" cy="10" r="8"/><path d="M10 9v5M10 6.2v.1"/>`,neutral:x`<circle cx="10" cy="10" r="3.2" class="fill"/>`},wt=new Set([`ok`,`warning`,`error`,`info`,`neutral`]),Tt=class extends C{constructor(){super(...arguments),this.status=`neutral`}static{this.styles=xt}get kind(){return wt.has(this.status)?this.status:`neutral`}render(){let e=this.kind;return b`<span class="status ${e}" part="status">
      <span class="icon" part="icon" aria-hidden="true"
        ><slot name="icon"><svg viewBox="0 0 20 20" focusable="false">${Ct[e]}</svg></slot></span
      ><span class="label"><slot></slot></span>
    </span>`}};St([T({reflect:!0})],Tt.prototype,`status`,void 0),Tt=St([w(`lit-material-status`)],Tt);var Et=o`
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
`,j=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},M=class extends C{constructor(){super(...arguments),this.label=``,this.value=``,this.caption=``,this.trendLabel=``,this.trendSentiment=`neutral`,this.variant=`outlined`}static{this.styles=Et}arrow(){switch(this.trend){case`up`:return`↑`;case`down`:return`↓`;case`flat`:return`→`;default:return``}}render(){let e=this.trend&&this.trendLabel;return b`
      <div class="stat ${this.variant}" part="stat" role="group" aria-label=${this.label||S}>
        <div class="label" part="label"><slot name="icon"></slot><span>${this.label}</span></div>
        <div class="value" part="value">${this.value}</div>
        <div class="caption" part="caption">
          ${e?b`<span class="trend ${this.trendSentiment}"><span aria-hidden="true">${this.arrow()}</span> ${this.trendLabel}</span>`:S}
          <slot name="caption">${this.caption?b`<span>${this.caption}</span>`:S}</slot>
        </div>
      </div>
    `}};j([T()],M.prototype,`label`,void 0),j([T()],M.prototype,`value`,void 0),j([T()],M.prototype,`caption`,void 0),j([T()],M.prototype,`trend`,void 0),j([T({attribute:`trend-label`})],M.prototype,`trendLabel`,void 0),j([T({attribute:`trend-sentiment`})],M.prototype,`trendSentiment`,void 0),j([T({reflect:!0})],M.prototype,`variant`,void 0),M=j([w(`lit-material-stat`)],M);var Dt=o`
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
`,Ot=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},kt=class extends C{constructor(){super(...arguments),this.headline=``,this.level=2,this.size=`standard`,this.emptyIcon=!1,this.emptyActions=!1}static{this.styles=Dt}onSlotChange(e){let t=e.target,n=t.assignedNodes({flatten:!0}).length===0;t.name===`icon`?this.emptyIcon=n:this.emptyActions=n}firstUpdated(){for(let e of this.renderRoot.querySelectorAll(`slot[name]`))e.dispatchEvent(new Event(`slotchange`))}render(){let e=Math.min(6,Math.max(1,Math.round(this.level||2)));return b`
      <div class="empty-state ${this.size}" part="empty-state">
        <div class="icon ${this.emptyIcon?`empty`:``}"><slot name="icon" @slotchange=${this.onSlotChange}></slot></div>
        ${this.headline?b`<div class="headline" part="headline" role="heading" aria-level=${e}>${this.headline}</div>`:S}
        <div class="body"><slot></slot></div>
        <div class="actions ${this.emptyActions?`empty`:``}"><slot name="actions" @slotchange=${this.onSlotChange}></slot></div>
      </div>
    `}};Ot([T()],kt.prototype,`headline`,void 0),Ot([T({type:Number})],kt.prototype,`level`,void 0),Ot([T({reflect:!0})],kt.prototype,`size`,void 0),Ot([E()],kt.prototype,`emptyIcon`,void 0),Ot([E()],kt.prototype,`emptyActions`,void 0),kt=Ot([w(`lit-material-empty-state`)],kt);var At={ATTRIBUTE:1,CHILD:2,PROPERTY:3,BOOLEAN_ATTRIBUTE:4,EVENT:5,ELEMENT:6},jt=e=>(...t)=>({_$litDirective$:e,values:t}),Mt=class{constructor(e){}get _$AU(){return this._$AM._$AU}_$AT(e,t,n){this._$Ct=e,this._$AM=t,this._$Ci=n}_$AS(e,t){return this.update(e,t)}update(e,t){return this.render(...t)}},{I:Nt}=Be,Pt=e=>e,Ft=()=>document.createComment(``),It=(e,t,n)=>{let r=e._$AA.parentNode,i=t===void 0?e._$AB:t._$AA;if(n===void 0)n=new Nt(r.insertBefore(Ft(),i),r.insertBefore(Ft(),i),e,e.options);else{let t=n._$AB.nextSibling,a=n._$AM,o=a!==e;if(o){let t;n._$AQ?.(e),n._$AM=e,n._$AP!==void 0&&(t=e._$AU)!==a._$AU&&n._$AP(t)}if(t!==i||o){let e=n._$AA;for(;e!==t;){let t=Pt(e).nextSibling;Pt(r).insertBefore(e,i),e=t}}}return n},Lt=(e,t,n=e)=>(e._$AI(t,n),e),Rt={},zt=(e,t=Rt)=>e._$AH=t,Bt=e=>e._$AH,Vt=e=>{e._$AR(),e._$AA.remove()},Ht=(e,t,n)=>{let r=new Map;for(let i=t;i<=n;i++)r.set(e[i],i);return r},Ut=jt(class extends Mt{constructor(e){if(super(e),e.type!==At.CHILD)throw Error(`repeat() can only be used in text expressions`)}dt(e,t,n){let r;n===void 0?n=t:t!==void 0&&(r=t);let i=[],a=[],o=0;for(let t of e)i[o]=r?r(t,o):o,a[o]=n(t,o),o++;return{values:a,keys:i}}render(e,t,n){return this.dt(e,t,n).values}update(e,[t,n,r]){let i=Bt(e),{values:a,keys:o}=this.dt(t,n,r);if(!Array.isArray(i))return this.ut=o,a;let s=this.ut??=[],c=[],l,u,d=0,f=i.length-1,p=0,m=a.length-1;for(;d<=f&&p<=m;)if(i[d]===null)d++;else if(i[f]===null)f--;else if(s[d]===o[p])c[p]=Lt(i[d],a[p]),d++,p++;else if(s[f]===o[m])c[m]=Lt(i[f],a[m]),f--,m--;else if(s[d]===o[m])c[m]=Lt(i[d],a[m]),It(e,c[m+1],i[d]),d++,m--;else if(s[f]===o[p])c[p]=Lt(i[f],a[p]),It(e,i[d],i[f]),f--,p++;else if(l===void 0&&(l=Ht(o,p,m),u=Ht(s,d,f)),l.has(s[d])){if(l.has(s[f])){let t=u.get(o[p]),n=t===void 0?null:i[t];if(n===null){let t=It(e,i[d]);Lt(t,a[p]),c[p]=t}else c[p]=Lt(n,a[p]),It(e,i[d],n),i[t]=null;p++}else Vt(i[f]),f--}else Vt(i[d]),d++;for(;p<=m;){let t=It(e,c[m+1]);Lt(t,a[p]),c[p++]=t}for(;d<=f;){let e=i[d++];e!==null&&Vt(e)}return this.ut=o,zt(e,c),Ee}}),Wt=o`
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
`,N=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},P=class extends C{constructor(){super(...arguments),this.maxLines=1e3,this.paused=!1,this.wrap=!1,this.label=`Log`,this.entries=[],this.unseen=0,this.held=[],this.nextId=0,this.following=!0}static{this.styles=Wt}get lines(){return this.entries.map(e=>e.text)}set lines(e){this.entries=[],this.held=[],this.unseen=0,this.following=!0,this.add(e)}appendLines(e){let t=(Array.isArray(e)?e:[e]).flatMap(e=>String(e).split(`
`));this.paused?this.held=[...this.held,...t].slice(-this.maxLines):this.add(t)}clear(){this.entries=[],this.held=[],this.unseen=0}scrollToLatest(){this.following=!0,this.unseen=0;let e=this.viewport;e&&(e.scrollTop=e.scrollHeight)}add(e){if(e.length===0)return;let t=e.map(e=>({id:this.nextId++,text:e,level:this.highlight?.(e)??void 0})),n=[...this.entries,...t],r=Math.max(0,n.length-this.maxLines);this.entries=r?n.slice(r):n,r&&this.dispatchEvent(new CustomEvent(`lines-dropped`,{detail:{count:r},bubbles:!0,composed:!0})),this.following||(this.unseen+=t.length)}willUpdate(e){if(e.has(`paused`)&&!this.paused&&this.held.length){let e=this.held;this.held=[],this.add(e)}}updated(){this.following&&this.viewport&&(this.viewport.scrollTop=this.viewport.scrollHeight)}onScroll(){let e=this.viewport;e&&(this.following=e.scrollHeight-e.scrollTop-e.clientHeight<24,this.following&&(this.unseen=0))}render(){return b`
      <div class="box">
        <div class="viewport" part="viewport" role="log" aria-live="off" aria-label=${this.label} tabindex="0" @scroll=${this.onScroll}>
          ${Ut(this.entries,e=>e.id,e=>b`<div class="line ${e.level??``}" part="line">${e.level?b`<span class="mark" aria-label=${e.level}></span>`:S}${e.text}</div>`)}
        </div>
        ${this.unseen>0?b`<button class="jump" part="jump" @click=${()=>this.scrollToLatest()}>${this.unseen} new line${this.unseen===1?``:`s`} ↓</button>`:S}
        ${this.paused&&this.held.length?b`<div class="held" role="status">Paused: ${this.held.length} line${this.held.length===1?``:`s`} waiting</div>`:S}
      </div>
    `}};N([T({type:Number,attribute:`max-lines`})],P.prototype,`maxLines`,void 0),N([T({type:Boolean,reflect:!0})],P.prototype,`paused`,void 0),N([T({type:Boolean,reflect:!0})],P.prototype,`wrap`,void 0),N([T()],P.prototype,`label`,void 0),N([T({attribute:!1})],P.prototype,`highlight`,void 0),N([E()],P.prototype,`entries`,void 0),N([E()],P.prototype,`unseen`,void 0),N([E()],P.prototype,`held`,void 0),N([D(`.viewport`)],P.prototype,`viewport`,void 0),P=N([w(`lit-material-log-viewer`)],P);var Gt=o`
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
`,Kt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},F=class extends C{constructor(){super(...arguments),this.columns=[],this.rows=[],this.caption=``,this.captionHidden=!1,this.density=`standard`,this.stickyHeader=!1}static{this.styles=Gt}cell(e,t){if(t.render)return t.render(e);let n=e[t.key];return n==null?``:String(n)}render(){let e=this.columns;return b`
      <div class="container" part="container" tabindex=${this.stickyHeader?`0`:S}>
        <table part="table">
          ${this.caption?b`<caption class=${this.captionHidden?`visually-hidden`:``}>${this.caption}</caption>`:S}
          <thead>
            <tr>
              ${e.map(e=>b`<th scope="col" class=${[e.numeric?`num`:``,e.hiddenLabel?`hidden-label`:``].join(` `).trim()||S}>
                    ${e.hiddenLabel?b`<span class="visually-hidden">${e.label}</span>`:e.label}
                  </th>`)}
            </tr>
          </thead>
          ${this.rows.length?b`<tbody>
                ${this.rows.map(t=>b`<tr>
                      ${e.map(e=>b`<td class=${[e.numeric?`num`:``,e.mono?`mono`:``].join(` `).trim()||S}>${this.cell(t,e)}</td>`)}
                    </tr>`)}
              </tbody>`:S}
        </table>
        ${this.rows.length?S:b`<div class="empty"><slot name="empty"></slot></div>`}
      </div>
    `}};Kt([T({attribute:!1})],F.prototype,`columns`,void 0),Kt([T({attribute:!1})],F.prototype,`rows`,void 0),Kt([T()],F.prototype,`caption`,void 0),Kt([T({type:Boolean,attribute:`caption-hidden`})],F.prototype,`captionHidden`,void 0),Kt([T({reflect:!0})],F.prototype,`density`,void 0),Kt([T({type:Boolean,reflect:!0,attribute:`sticky-header`})],F.prototype,`stickyHeader`,void 0),F=Kt([w(`lit-material-table`)],F);function qt(e,t){if(!(e>0)||!Number.isFinite(e))return 1;let n=e/Math.max(1,t),r=10**Math.floor(Math.log10(n));for(let e of[1,2,2.5,5,10])if(e*r>=n)return e*r;return 10*r}function Jt(e,t,n=4){if(!Number.isFinite(e)||!Number.isFinite(t))return{ticks:[0,1],min:0,max:1};if(e===t){let n=e===0?1:Math.abs(e)*.1;e-=n,t+=n}let r=qt(t-e,n),i=Math.floor(e/r)*r,a=Math.ceil(t/r)*r,o=[];for(let e=i,t=0;e<=a+r/2&&t<n*3+2;e+=r,t++)o.push(Number(e.toPrecision(12)));return{ticks:o,min:i,max:a}}function Yt(e,t,n=5){return!Number.isFinite(e)||!Number.isFinite(t)?[]:e===t?[e]:Array.from({length:n},(r,i)=>e+(t-e)*i/(n-1))}function Xt(e){if(!Number.isFinite(e))return``;let t=Math.abs(e);return t>=1e9?`${Zt(e/1e9)}G`:t>=1e6?`${Zt(e/1e6)}M`:t>=1e4?`${Zt(e/1e3)}k`:t>=100||Number.isInteger(e)?String(Math.round(e)):Zt(e)}function Zt(e){return String(Number(e.toPrecision(3)))}function Qt(e,t,n,r){let i=e.map((e,t)=>({p:e,i:t})).sort((e,t)=>e.p-t.p),a=[...e],o=-1/0;for(let{p:e,i:r}of i){let i=Math.max(e,o+t,n);a[r]=i,o=i}let s=Math.max(0,Math.max(...a)-r);return s?a.map(e=>e-s):a}var $t=a([[`#2a78d6`,`#3987e5`],[`#eb6834`,`#d95926`],[`#1baf7a`,`#199e70`],[`#eda100`,`#c98500`],[`#e87ba4`,`#d55181`],[`#008300`,`#008300`],[`#6250d6`,`#9085e9`],[`#e34948`,`#e66767`]].map(([e,t],n)=>`.s${n+1} { --series: var(--lit-material-chart-series-${n+1}, light-dark(${e}, ${t})); }`).join(`
`)),en=o`
  :host {
    --chart-text: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    --chart-muted: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    --chart-grid: var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    --chart-surface: var(--md-sys-color-surface, light-dark(#fef7ff, #141218));
  }
`,tn=[en,$t,o`
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
  `],I=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},nn=4,rn=8,an=28,on=40,L=class extends C{constructor(){super(...arguments),this.series=[],this.label=``,this.height=240,this.formatX=Xt,this.formatY=Xt,this.unit=``,this.width=640,this.active=-1}static{this.styles=tn}connectedCallback(){super.connectedCallback(),typeof ResizeObserver<`u`&&(this.observer=new ResizeObserver(e=>{let t=Math.round(e[0]?.contentRect.width??0);t>0&&t!==this.width&&requestAnimationFrame(()=>this.width=t)}),this.observer.observe(this))}disconnectedCallback(){super.disconnectedCallback(),this.observer?.disconnect()}get drawn(){return this.series.length>8&&console.warn(`lit-material-line-chart: ${this.series.length} series; only the first 8 are drawn`),this.series.slice(0,8).map(e=>({name:e.name,points:e.points.filter(e=>Number.isFinite(e.x)&&Number.isFinite(e.y)).sort((e,t)=>e.x-t.x)}))}xsOf(e){let t=new Set;for(let n of e)for(let e of n.points)t.add(e.x);return[...t].sort((e,t)=>e-t)}nearest(e,t){let n;for(let r of e)(!n||Math.abs(r.x-t)<Math.abs(n.x-t))&&(n=r);return n}onPointer(e,t,n){let r=e.currentTarget.getBoundingClientRect(),i=(e.clientX-r.left)/Math.max(1,r.width)*this.width,a=0;for(let e=1;e<t.length;e++)Math.abs(n(t[e])-i)<Math.abs(n(t[a])-i)&&(a=e);this.active=a}onKey(e,t){let n={ArrowLeft:-1,ArrowRight:1,Home:-1/0,End:1/0};if(e.key===`Escape`){this.active=-1;return}if(!(e.key in n))return;e.preventDefault();let r=n[e.key],i=this.active<0?t-1:this.active;this.active=Math.min(t-1,Math.max(0,Number.isFinite(r)?i+r:r<0?0:t-1))}summary(e){return e.map(e=>{let t=e.points.map(e=>e.y);if(!t.length)return`${e.name}: no data`;let n=t[t.length-1];return`${e.name}: latest ${this.formatY(n)}${this.unit?` ${this.unit}`:``}, range ${this.formatY(Math.min(...t))} to ${this.formatY(Math.max(...t))}`}).join(`; `)}render(){let e=this.drawn,t=this.xsOf(e),n=this.label||`Line chart`;if(t.length===0)return b`<div class="empty" role="img" aria-label="${n}: no data">No data</div>`;let r=e.flatMap(e=>e.points.map(e=>e.y)),i=Math.min(...r),a=this.yMin??Math.min(0,i),{ticks:o,min:s,max:c}=Jt(Math.min(a,i),Math.max(...r),4),l=e.length>1&&e.length<=nn,u=Math.max(0,...e.map(e=>e.name.length)),d=l?Math.min(160,12+u*7):12,f=o.map(e=>this.formatY(e)),p=Math.max(on,12+Math.max(...f.map(e=>e.length))*7),m=this.width,h=Math.max(80,this.height),ee=Math.max(1,m-p-d),te=Math.max(1,h-rn-an),ne=t[0],re=t[t.length-1],g=e=>p+(re===ne?ee/2:(e-ne)/(re-ne)*ee),_=e=>rn+(1-(e-s)/(c-s||1))*te,ie=Yt(ne,re,Math.max(2,Math.min(6,Math.floor(ee/110)+1))),ae=e.map(e=>e.points.length?_(e.points[e.points.length-1].y):rn),oe=l?Qt(ae,14,14,rn+te):[],v=this.active>=0&&this.active<t.length?t[this.active]:void 0,se=v===void 0?[]:e.map((e,t)=>({i:t,name:e.name,point:this.nearest(e.points,v)})).filter(e=>e.point),ce=v===void 0?0:g(v)/m*100;return b`
      ${e.length>1?b`<ul class="legend" part="legend" aria-hidden="true">
            ${e.map((e,t)=>b`<li><span class="swatch s${t+1}"></span>${e.name}</li>`)}
          </ul>`:S}
      <div class="chart" part="chart">
        <svg
          viewBox="0 0 ${m} ${h}"
          width=${m}
          height=${h}
          role="img"
          aria-label="${n}. ${this.summary(e)}"
          tabindex="0"
          @pointermove=${e=>this.onPointer(e,t,g)}
          @pointerdown=${e=>this.onPointer(e,t,g)}
          @pointerleave=${()=>this.active=-1}
          @keydown=${e=>this.onKey(e,t.length)}
          @blur=${()=>this.active=-1}
        >
          ${o.map((e,t)=>x`
              <line class="grid" x1=${p} x2=${p+ee} y1=${_(e)} y2=${_(e)}></line>
              <text class="axis" x=${p-8} y=${_(e)+4} text-anchor="end">${f[t]}</text>`)}
          ${ie.map((e,t)=>x`<text class="axis" x=${g(e)} y=${h-8}
              text-anchor=${t===0?`start`:t===ie.length-1?`end`:`middle`}>${this.formatX(e)}</text>`)}
          ${e.map((e,t)=>e.points.length===1?x`<circle class="dot s${t+1}" r="4" cx=${g(e.points[0].x)} cy=${_(e.points[0].y)}></circle>`:x`<polyline class="line s${t+1}" points=${e.points.map(e=>`${g(e.x).toFixed(1)},${_(e.y).toFixed(1)}`).join(` `)}></polyline>`)}
          ${l?e.map((e,t)=>x`<text class="end-label" x=${p+ee+6} y=${(oe[t]??rn)+4}>${e.name}</text>`):S}
          ${v===void 0?S:x`<line class="crosshair" x1=${g(v)} x2=${g(v)} y1=${rn} y2=${rn+te}></line>
                ${se.map(e=>x`<circle class="marker s${e.i+1}" r="4" cx=${g(e.point.x)} cy=${_(e.point.y)}></circle>`)}`}
        </svg>
        ${v===void 0?S:b`<div class="tooltip" part="tooltip" role="status" style="left: ${ce}%">
              <div class="tip-x">${this.formatX(v)}</div>
              ${se.map(t=>b`<div class="tip-row">${e.length>1?b`<span class="swatch s${t.i+1}"></span>${t.name}: `:S}<strong
                    >${this.formatY(t.point.y)}</strong
                  >${this.unit?` ${this.unit}`:``}</div>`)}
            </div>`}
      </div>
      <table class="visually-hidden">
        <caption>${n}</caption>
        <thead>
          <tr>
            <th scope="col">x</th>
            ${e.map(e=>b`<th scope="col">${e.name}${this.unit?` (${this.unit})`:``}</th>`)}
          </tr>
        </thead>
        <tbody>
          ${t.slice(-200).map(t=>b`<tr>
                <th scope="row">${this.formatX(t)}</th>
                ${e.map(e=>{let n=e.points.find(e=>e.x===t);return b`<td>${n?this.formatY(n.y):``}</td>`})}
              </tr>`)}
        </tbody>
      </table>
    `}};I([T({attribute:!1})],L.prototype,`series`,void 0),I([T()],L.prototype,`label`,void 0),I([T({type:Number})],L.prototype,`height`,void 0),I([T({type:Number,attribute:`y-min`})],L.prototype,`yMin`,void 0),I([T({attribute:!1})],L.prototype,`formatX`,void 0),I([T({attribute:!1})],L.prototype,`formatY`,void 0),I([T()],L.prototype,`unit`,void 0),I([E()],L.prototype,`width`,void 0),I([E()],L.prototype,`active`,void 0),L=I([w(`lit-material-line-chart`)],L);var sn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},cn=class extends C{constructor(){super(...arguments),this.values=[],this.label=``,this.showLast=!1,this.format=Xt}static{this.styles=[en,$t,o`
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
    `]}render(){let e=this.values.filter(e=>Number.isFinite(e)),t=this.label||`Trend`;if(e.length===0)return b`<svg role="img" aria-label="${t}: no data"></svg>`;let n=Math.min(...e),r=Math.max(...e),i=r-n||1,a=t=>e.length===1?50:t/(e.length-1)*100,o=e=>2+(1-(e-n)/i)*20,s=e[e.length-1];return b`<svg class="s1" viewBox="0 0 ${100} ${24}" preserveAspectRatio="none" role="img"
      aria-label="${t}: latest ${this.format(s)}, from ${this.format(n)} to ${this.format(r)}">
      <polyline points=${e.map((e,t)=>`${a(t).toFixed(2)},${o(e).toFixed(2)}`).join(` `)}></polyline>
      ${this.showLast||e.length===1?x`<circle cx=${a(e.length-1)} cy=${o(s)} r="2.5"></circle>`:S}
    </svg>`}};sn([T({attribute:!1})],cn.prototype,`values`,void 0),sn([T()],cn.prototype,`label`,void 0),sn([T({type:Boolean,attribute:`show-last`})],cn.prototype,`showLast`,void 0),sn([T({attribute:!1})],cn.prototype,`format`,void 0),cn=sn([w(`lit-material-sparkline`)],cn);var ln=[en,o`
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
  `],R=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},z=class extends C{constructor(){super(...arguments),this.data=[],this.label=``,this.highlight=``,this.sort=``,this.unit=``,this.format=Xt,this.active=-1}static{this.styles=ln}get rows(){let e=this.data.filter(e=>Number.isFinite(e.value));return this.sort===`desc`?e.sort((e,t)=>t.value-e.value):this.sort===`asc`&&e.sort((e,t)=>e.value-t.value),e}text(e){return`${this.format(e.value)}${this.unit?` ${this.unit}`:``}`}onKey(e,t){if(e.key===`Escape`){this.active=-1;return}let n={ArrowDown:e=>e+1,ArrowUp:e=>e<0?t-1:e-1,Home:()=>0,End:()=>t-1}[e.key];n&&(e.preventDefault(),this.active=Math.min(t-1,Math.max(0,n(this.active))))}render(){let e=this.rows,t=this.label||`Bar chart`;if(e.length===0)return b`<div class="empty" role="img" aria-label="${t}: no data">No data</div>`;let n=Math.max(0,...e.map(e=>e.value))||1,r=this.highlight!==``&&e.some(e=>e.name===this.highlight),i=e[this.active];return b`
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
        ${e.map((e,t)=>{let i=r&&e.name===this.highlight,a=Math.max(0,e.value)/n;return b`<div class=${[`row`,i?`highlight`:``,r&&!i?`muted`:``,t===this.active?`active`:``].join(` `)} aria-hidden="true" @pointerenter=${()=>this.active=t} @pointerdown=${()=>this.active=t}>
            <span class="name">${e.name}</span>
            <span class="track" style="--f: ${a.toFixed(4)}">
              <span class="bar"></span>
              ${!r||i?b`<span class="value">${this.text(e)}</span>`:S}
            </span>
          </div>`})}
        ${i?b`<div class="tooltip" part="tooltip" role="status" style="top: calc(${this.active} * var(--row-pitch))">
              ${i.name}: <strong>${this.text(i)}</strong>
            </div>`:S}
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
          ${e.map(e=>b`<tr><th scope="row">${e.name}</th><td>${this.format(e.value)}</td></tr>`)}
        </tbody>
      </table>
    `}};R([T({attribute:!1})],z.prototype,`data`,void 0),R([T()],z.prototype,`label`,void 0),R([T()],z.prototype,`highlight`,void 0),R([T()],z.prototype,`sort`,void 0),R([T()],z.prototype,`unit`,void 0),R([T({attribute:!1})],z.prototype,`format`,void 0),R([E()],z.prototype,`active`,void 0),z=R([w(`lit-material-bar-chart`)],z);var un=o`
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
`,dn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},fn=class extends C{constructor(){super(...arguments),this.variant=`text`,this.animation=`pulse`,this.width=``,this.height=``}static{this.styles=un}render(){return b`<span class="skeleton" part="skeleton" style=${`${this.width?`width: ${this.width};`:``}${this.height?`height: ${this.height};`:``}`||S} aria-hidden="true"></span>`}};dn([T({reflect:!0})],fn.prototype,`variant`,void 0),dn([T({reflect:!0})],fn.prototype,`animation`,void 0),dn([T()],fn.prototype,`width`,void 0),dn([T()],fn.prototype,`height`,void 0),fn=dn([w(`lit-material-skeleton`)],fn);var pn=o`
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
`,mn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},hn=class extends C{static{this.styles=pn}constructor(){super(),this.open=!1,this.duration=5e3,this.closable=!1,this.pauseTimer=()=>{this.open&&this.clearTimer()},this.resumeTimer=()=>{this.open&&this.startTimer()},this.handleClick=e=>{e.target.closest(`[slot="action"]`)&&this.close()},this.addEventListener(`click`,this.handleClick),this.addEventListener(`pointerenter`,this.pauseTimer),this.addEventListener(`pointerleave`,this.resumeTimer),this.addEventListener(`focusin`,this.pauseTimer),this.addEventListener(`focusout`,this.resumeTimer)}connectedCallback(){super.connectedCallback(),this.setAttribute(`popover`,`manual`),this.setAttribute(`role`,`status`),this.setAttribute(`aria-live`,`polite`),this.setAttribute(`aria-atomic`,`true`)}disconnectedCallback(){super.disconnectedCallback(),this.clearTimer()}show(e){if(e!==void 0){for(let e of[...this.childNodes])e.nodeType===Node.TEXT_NODE&&e.remove();this.prepend(document.createTextNode(e))}this.open=!0}close(){this.open=!1}updated(e){e.has(`open`)&&(this.open?(this.matches(`:popover-open`)||this.showPopover(),this.startTimer()):this.matches(`:popover-open`)&&(this.clearTimer(),this.hidePopover(),this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0}))))}render(){return b`
      <div class="content" part="snackbar">
        <slot></slot>
      </div>
      <slot name="action" class="action"></slot>
      ${this.closable?b`
            <button class="close" part="close-button" type="button" aria-label="Dismiss" @click=${this.close}>
              <svg viewBox="0 0 18 18" aria-hidden="true">
                <path d="M5 5 L13 13 M13 5 L5 13"></path>
              </svg>
            </button>
          `:S}
    `}startTimer(){this.clearTimer(),this.duration>0&&(this.timeoutId=window.setTimeout(()=>this.close(),this.duration))}clearTimer(){this.timeoutId!==void 0&&(window.clearTimeout(this.timeoutId),this.timeoutId=void 0)}};mn([T({type:Boolean,reflect:!0})],hn.prototype,`open`,void 0),mn([T({type:Number})],hn.prototype,`duration`,void 0),mn([T({type:Boolean})],hn.prototype,`closable`,void 0),hn=mn([w(`lit-material-snackbar`)],hn);var gn=o`
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
`,B=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},V=class extends C{static{this.styles=gn}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.checked=!1,this.disabled=!1,this.error=!1,this.required=!1,this.name=``,this.value=`on`,this.ariaLabel=null,this.ariaLabelledBy=null,this.ripple=new Qe(this),this.focusRing=new $e(this),this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement),this.syncInternals()}updated(e){(e.has(`checked`)||e.has(`required`)||e.has(`error`))&&this.syncInternals()}syncInternals(){if(this.internals.setFormValue(this.checked?this.value:null),this.inputElement){let e=this.error?this.inputElement.validationMessage||`Invalid`:this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){return b`
      <div class="switch ${this.checked?`checked`:``}" part="switch">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="track" part="track">
          <div class="thumb-wrap">
            <div class="state-layer" part="ripple"></div>
            <div class="thumb" part="thumb">
              ${this.checked?b`<slot name="selected-icon" class="icon"></slot>`:b`<slot name="icon" class="icon"></slot>`}
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
          name=${this.name||S}
          value=${this.value||S}
          aria-invalid=${this.error?`true`:S}
          aria-label=${this.ariaLabel||S}
          aria-labelledby=${this.ariaLabelledBy||S}
          @change=${this.handleChange}
        />
      </div>
    `}handleChange(e){let t=e.target;this.checked=t.checked,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}formResetCallback(){this.checked=this.hasAttribute(`checked`),this.syncInternals()}formStateRestoreCallback(e){this.checked=e!==null}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};B([T({type:Boolean})],V.prototype,`checked`,void 0),B([T({type:Boolean,reflect:!0})],V.prototype,`disabled`,void 0),B([T({type:Boolean,reflect:!0})],V.prototype,`error`,void 0),B([T({type:Boolean})],V.prototype,`required`,void 0),B([T()],V.prototype,`name`,void 0),B([T()],V.prototype,`value`,void 0),B([T({reflect:!0})],V.prototype,`form`,void 0),B([T({attribute:`aria-label`})],V.prototype,`ariaLabel`,void 0),B([T({attribute:`aria-labelledby`})],V.prototype,`ariaLabelledBy`,void 0),B([D(`.native-control`)],V.prototype,`inputElement`,void 0),B([D(`.state-layer`)],V.prototype,`stateLayerElement`,void 0),V=B([w(`lit-material-switch`)],V);var _n=o`
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
`,H=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},vn=0,U=class extends C{static{this.styles=_n}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`text`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.prefix=``,this.suffix=``,this.step=``,this.pattern=``,this.autocomplete=``,this.inputmode=``,this.name=``,this.focused=!1,this.touched=!1,this.focusRing=new $e(this),this.labelId=`lit-material-text-field-label-${++vn}`,this.supportingId=`lit-material-text-field-supporting-${vn}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`pattern`)||e.has(`minlength`)||e.has(`maxlength`)||e.has(`min`)||e.has(`max`)||e.has(`type`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.inputElement?!1:!this.inputElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.inputElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.inputElement){let e=this.isError&&this.errorText||this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){let e=this.floated,t=this.isError;return b`
      <div class="text-field" part="text-field">
        <div
          class="container ${e?`floated`:``} ${this.focused?`focused`:``}"
          part="container"
        >
          <slot name="leading-icon" class="leading-icon"></slot>
          <div class="field">
            ${this.prefix?b`<span class="prefix">${this.prefix}</span>`:S}
            <div class="input-wrapper">
              ${this.label?b`<label class="label" part="label" id=${this.labelId} for="input">${this.label}</label>`:S}
              <input
                class="input"
                part="input"
                id="input"
                .value=${this.value}
                type=${this.type||`text`}
                placeholder=${this.placeholder||S}
                ?disabled=${this.disabled}
                ?readonly=${this.readonly}
                ?required=${this.required}
                autocomplete=${this.autocomplete||S}
                inputmode=${this.inputmode||S}
                min=${this.min??S}
                max=${this.max??S}
                minlength=${this.minlength??S}
                maxlength=${this.maxlength??S}
                step=${this.step||S}
                pattern=${this.pattern||S}
                name=${this.name||S}
                aria-labelledby=${this.label?this.labelId:S}
                aria-describedby=${this.supportingMessage?this.supportingId:S}
                aria-invalid=${t?`true`:S}
                @input=${this.handleInput}
                @change=${this.handleChange}
                @focus=${this.handleFocus}
                @blur=${this.handleBlur}
                @keydown=${this.handleKeydown}
              />
            </div>
            ${this.suffix?b`<span class="suffix">${this.suffix}</span>`:S}
          </div>
          <slot name="trailing-icon" class="trailing-icon"></slot>
        </div>
        ${this.supportingMessage||this.maxlength!=null?b`
              <div class="supporting" part="supporting">
                <span class="supporting-text" id=${this.supportingId}>
                  ${this.supportingMessage}
                </span>
                ${this.maxlength==null?S:b`<span class="counter">${this.value.length} / ${this.maxlength}</span>`}
              </div>
            `:S}
      </div>
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleKeydown(e){if(e.key!==`Enter`||e.isComposing||e.defaultPrevented)return;let t=this.internals.form;t&&(e.preventDefault(),t.requestSubmit())}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};H([T()],U.prototype,`variant`,void 0),H([T()],U.prototype,`type`,void 0),H([T()],U.prototype,`value`,void 0),H([T()],U.prototype,`label`,void 0),H([T()],U.prototype,`placeholder`,void 0),H([T({attribute:`supporting-text`})],U.prototype,`supportingText`,void 0),H([T({attribute:`error-text`})],U.prototype,`errorText`,void 0),H([T({type:Boolean,reflect:!0})],U.prototype,`error`,void 0),H([T({type:Boolean,reflect:!0})],U.prototype,`disabled`,void 0),H([T({type:Boolean})],U.prototype,`required`,void 0),H([T({type:Boolean})],U.prototype,`readonly`,void 0),H([T()],U.prototype,`prefix`,void 0),H([T()],U.prototype,`suffix`,void 0),H([T({type:Number})],U.prototype,`min`,void 0),H([T({type:Number})],U.prototype,`max`,void 0),H([T({type:Number})],U.prototype,`minlength`,void 0),H([T({type:Number})],U.prototype,`maxlength`,void 0),H([T()],U.prototype,`step`,void 0),H([T()],U.prototype,`pattern`,void 0),H([T()],U.prototype,`autocomplete`,void 0),H([T()],U.prototype,`inputmode`,void 0),H([T()],U.prototype,`name`,void 0),H([T({reflect:!0})],U.prototype,`form`,void 0),H([D(`.input`)],U.prototype,`inputElement`,void 0),H([E()],U.prototype,`focused`,void 0),H([E()],U.prototype,`touched`,void 0),U=H([w(`lit-material-text-field`)],U);var yn=o`
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
`,W=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},bn=0,G=class extends C{static{this.styles=yn}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.rows=4,this.resize=`vertical`,this.autocomplete=``,this.name=``,this.focused=!1,this.touched=!1,this.labelId=`lit-material-textarea-label-${++bn}`,this.supportingId=`lit-material-textarea-supporting-${bn}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`minlength`)||e.has(`maxlength`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.textareaElement?!1:!this.textareaElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.textareaElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.textareaElement){let e=this.isError&&this.errorText||this.textareaElement.validationMessage;this.internals.setValidity(this.textareaElement.validity,e,this.textareaElement)}}render(){let e=this.floated,t=this.isError;return b`
      <div class="textarea-field" part="textarea-field">
        <div class="container ${e?`floated`:``} ${this.focused?`focused`:``}" part="container">
          ${this.label?b`<label class="label" part="label" id=${this.labelId} for="textarea">${this.label}</label>`:S}
          <textarea
            class="textarea"
            part="textarea"
            id="textarea"
            style="resize: ${this.resize}"
            rows=${this.rows}
            .value=${this.value}
            placeholder=${this.placeholder||S}
            ?disabled=${this.disabled}
            ?readonly=${this.readonly}
            ?required=${this.required}
            autocomplete=${this.autocomplete||S}
            minlength=${this.minlength??S}
            maxlength=${this.maxlength??S}
            name=${this.name||S}
            aria-labelledby=${this.label?this.labelId:S}
            aria-describedby=${this.supportingMessage?this.supportingId:S}
            aria-invalid=${t?`true`:S}
            @input=${this.handleInput}
            @change=${this.handleChange}
            @focus=${this.handleFocus}
            @blur=${this.handleBlur}
          ></textarea>
        </div>
        ${this.supportingMessage||this.maxlength!=null?b`
              <div class="supporting" part="supporting">
                <span class="supporting-text" id=${this.supportingId}> ${this.supportingMessage} </span>
                ${this.maxlength==null?S:b`<span class="counter">${this.value.length} / ${this.maxlength}</span>`}
              </div>
            `:S}
      </div>
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};W([T()],G.prototype,`variant`,void 0),W([T()],G.prototype,`value`,void 0),W([T()],G.prototype,`label`,void 0),W([T()],G.prototype,`placeholder`,void 0),W([T({attribute:`supporting-text`})],G.prototype,`supportingText`,void 0),W([T({attribute:`error-text`})],G.prototype,`errorText`,void 0),W([T({type:Boolean,reflect:!0})],G.prototype,`error`,void 0),W([T({type:Boolean,reflect:!0})],G.prototype,`disabled`,void 0),W([T({type:Boolean})],G.prototype,`required`,void 0),W([T({type:Boolean})],G.prototype,`readonly`,void 0),W([T({type:Number})],G.prototype,`minlength`,void 0),W([T({type:Number})],G.prototype,`maxlength`,void 0),W([T({type:Number})],G.prototype,`rows`,void 0),W([T()],G.prototype,`resize`,void 0),W([T()],G.prototype,`autocomplete`,void 0),W([T()],G.prototype,`name`,void 0),W([T({reflect:!0})],G.prototype,`form`,void 0),W([D(`.textarea`)],G.prototype,`textareaElement`,void 0),W([E()],G.prototype,`focused`,void 0),W([E()],G.prototype,`touched`,void 0),G=W([w(`lit-material-textarea`)],G);var xn=o`
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
`,K=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},q=class extends C{static{this.styles=xn}static{this.shadowRootOptions={...C.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`standard`,this.toggle=!1,this.selected=!1,this.disabled=!1,this.type=`button`,this.name=``,this.value=``,this.href=``,this.target=``,this.ariaLabel=null,this.ariaLabelledBy=null,this.ripple=new Qe(this),this.focusRing=new $e(this),this.internals=this.attachInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}render(){let e=this.toggle&&this.selected?b`<slot name="selected-icon" class="icon"></slot>`:b`<slot name="icon" class="icon"></slot><slot class="icon"></slot>`,t=b`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      ${e}
    `;return this.href?b`
        <a
          class="button"
          part="button"
          href=${this.disabled?S:this.href}
          target=${this.target||S}
          rel=${this.target===`_blank`?`noopener noreferrer`:S}
          aria-disabled=${this.disabled?`true`:S}
          aria-label=${this.ariaLabel||S}
          aria-labelledby=${this.ariaLabelledBy||S}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${t}
        </a>
      `:b`
      <button
        class="button"
        part="button"
        type="button"
        ?disabled=${this.disabled}
        name=${this.name||S}
        value=${this.value||S}
        aria-pressed=${this.toggle?this.selected:S}
        aria-label=${this.ariaLabel||S}
        aria-labelledby=${this.ariaLabelledBy||S}
        @click=${this.handleClick}
      >
        ${t}
      </button>
    `}handleClick(e){this.disabled?(e.preventDefault(),e.stopImmediatePropagation()):this.toggle?(this.selected=!this.selected,this.dispatchEvent(new Event(`change`,{bubbles:!0}))):this.type===`submit`?this.internals.form?.requestSubmit():this.type===`reset`&&this.internals.form?.reset()}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};K([T()],q.prototype,`variant`,void 0),K([T({type:Boolean,reflect:!0})],q.prototype,`toggle`,void 0),K([T({type:Boolean,reflect:!0})],q.prototype,`selected`,void 0),K([T({type:Boolean,reflect:!0})],q.prototype,`disabled`,void 0),K([T()],q.prototype,`type`,void 0),K([T()],q.prototype,`name`,void 0),K([T()],q.prototype,`value`,void 0),K([T({reflect:!0})],q.prototype,`form`,void 0),K([T()],q.prototype,`href`,void 0),K([T()],q.prototype,`target`,void 0),K([T({attribute:`aria-label`})],q.prototype,`ariaLabel`,void 0),K([T({attribute:`aria-labelledby`})],q.prototype,`ariaLabelledBy`,void 0),K([D(`.state-layer`)],q.prototype,`stateLayerElement`,void 0),q=K([w(`lit-material-icon-button`)],q);var J=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Sn=b`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19 13H5v-2h14v2z"></path></svg>`,Cn=b`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"></path></svg>`;function wn(e){let t=String(e),n=/e-(\d+)$/.exec(t);return n?Number(n[1]):t.includes(`.`)?t.length-t.indexOf(`.`)-1:0}var Y=class extends C{constructor(){super(...arguments),this.value=null,this.step=1,this.label=``,this.unit=``,this.variant=`outlined`,this.supportingText=``,this.errorText=``,this.name=``,this.required=!1,this.disabled=!1,this.readonly=!1,this.validationMessage=``,this.draft=null}static{this.styles=o`
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
  `}checkValidity(){return this.validationMessage=this.check(),!this.validationMessage}check(){return this.draft!==null&&this.draft.trim()!==``?`Enter a number`:this.value===null?this.required?`Required`:``:this.min!==void 0&&this.value<this.min?`At least ${this.min}`:this.max!==void 0&&this.value>this.max?`At most ${this.max}`:``}willUpdate(e){e.has(`value`)&&(this.draft=null),this.validationMessage=this.check()}clamp(e){return this.min!==void 0&&(e=Math.max(this.min,e)),this.max!==void 0&&(e=Math.min(this.max,e)),e}stepBy(e){if(this.value===null){this.commit(this.clamp(this.min??0));return}let t=this.value,n=Math.max(wn(this.step),wn(t));this.commit(this.clamp(Number((t+e*this.step).toFixed(n))))}commit(e){let t=e!==this.value||this.draft!==null;this.value=e,this.draft=null,t&&this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0}))}onInput(e){e.stopPropagation();let t=e.target.value.trim(),n=t===``?null:Number(t.replace(`,`,`.`));n!==null&&!Number.isFinite(n)?(this.draft=t,this.validationMessage=this.check()):(this.value=n,this.draft=null,this.dispatchEvent(new Event(`input`,{bubbles:!0,composed:!0})))}onKeyDown(e){if(this.disabled||this.readonly)return;let t={ArrowUp:1,ArrowDown:-1,PageUp:10,PageDown:-10};e.key in t?(e.preventDefault(),this.stepBy(t[e.key])):e.key===`Home`&&this.min!==void 0?(e.preventDefault(),this.commit(this.min)):e.key===`End`&&this.max!==void 0&&(e.preventDefault(),this.commit(this.max))}render(){let e=this.disabled||this.readonly,t=this.label||`value`,n=this.validationMessage||this.errorText,r=this.draft??(this.value===null?``:String(this.value));return b`
      <lit-material-icon-button
        part="decrement"
        aria-label=${`Decrease ${t}`}
        ?disabled=${e||this.min!==void 0&&this.value!==null&&this.value<=this.min}
        @click=${()=>this.stepBy(-1)}
        ><span slot="icon">${Sn}</span></lit-material-icon-button
      >
      <lit-material-text-field
        part="field"
        variant=${this.variant}
        label=${this.label}
        inputmode=${this.step%1==0&&(this.min??0)>=0?`numeric`:`decimal`}
        autocomplete="off"
        .value=${r}
        suffix=${this.unit||S}
        name=${this.name||S}
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
        ><span slot="icon">${Cn}</span></lit-material-icon-button
      >
    `}};J([T({type:Number})],Y.prototype,`value`,void 0),J([T({type:Number})],Y.prototype,`min`,void 0),J([T({type:Number})],Y.prototype,`max`,void 0),J([T({type:Number})],Y.prototype,`step`,void 0),J([T()],Y.prototype,`label`,void 0),J([T()],Y.prototype,`unit`,void 0),J([T()],Y.prototype,`variant`,void 0),J([T({attribute:`supporting-text`})],Y.prototype,`supportingText`,void 0),J([T({attribute:`error-text`})],Y.prototype,`errorText`,void 0),J([T()],Y.prototype,`name`,void 0),J([T({type:Boolean})],Y.prototype,`required`,void 0),J([T({type:Boolean,reflect:!0})],Y.prototype,`disabled`,void 0),J([T({type:Boolean})],Y.prototype,`readonly`,void 0),J([E()],Y.prototype,`validationMessage`,void 0),J([E()],Y.prototype,`draft`,void 0),Y=J([w(`lit-material-number-field`)],Y);var Tn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},En=class extends C{static{this.styles=o`
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
  `}willUpdate(){this.hasAttribute(`role`)||this.setAttribute(`role`,`group`)}render(){return b`<slot></slot>`}};En=Tn([w(`lit-material-input-group`)],En);var Dn=`rws-admin-token`,On=``;function kn(){try{return sessionStorage.getItem(Dn)??On}catch{return On}}function An(e){On=e;try{e?sessionStorage.setItem(Dn,e):sessionStorage.removeItem(Dn)}catch{}}var X=class extends Error{constructor(e,t){super(t),this.status=e}};async function Z(e,{method:t=`GET`,body:n}={}){let r=await fetch(`/admin/api/v1${e}`,{method:t,headers:{...kn()?{Authorization:`Bearer ${kn()}`}:{},"X-Rws-Admin":`1`,...n===void 0?{}:{"Content-Type":`application/json`}},credentials:`same-origin`,body:n===void 0?void 0:JSON.stringify(n),cache:`no-store`}),i=await r.text(),a=null;try{a=i?JSON.parse(i):null}catch{a=null}if(!r.ok)throw new X(r.status,a?.error??`${r.status} ${r.statusText}`);return a}async function jn(e,t){let n=await fetch(`/admin/api/v1/logs/stream`,{headers:{...kn()?{Authorization:`Bearer ${kn()}`}:{},"X-Rws-Admin":`1`},credentials:`same-origin`,signal:t,cache:`no-store`});if(!n.ok||!n.body)throw new X(n.status,`${n.status} ${n.statusText}`);let r=n.body.pipeThrough(new TextDecoderStream).getReader(),i=``;for(;;){let{value:t,done:n}=await r.read();if(n)return;i+=t;let a;for(;(a=i.indexOf(`

`))>=0;){let t=i.slice(0,a);if(i=i.slice(a+2),t.split(`
`).some(e=>e.startsWith(`event:`)))continue;let n=t.split(`
`).filter(e=>e.startsWith(`data:`)).map(e=>e.slice(5).replace(/^ /,``)).join(`
`);n&&e(n)}}}var Mn=[`B`,`KiB`,`MiB`,`GiB`,`TiB`,`PiB`,`EiB`],Nn=[`B`,`kB`,`MB`,`GB`,`TB`,`PB`,`EB`];function Pn(e,t={}){let{locale:n,maximumFractionDigits:r=1,...i}=t;return new Intl.NumberFormat(n,{maximumFractionDigits:r,...i}).format(e)}function Fn(e,t={}){if(!Number.isFinite(e))return``;let n=t.binary===!1?1e3:1024,r=t.binary===!1?Nn:Mn,i=Math.abs(e),a=0;for(;i>=n&&a<r.length-1;)i/=n,a++;let o=t.maximumFractionDigits??1;return a<r.length-1&&Number(i.toFixed(o))>=n&&(i/=n,a++),`${e<0?`-`:``}${Pn(i,{...t,maximumFractionDigits:a===0?0:o})} ${r[a]}`}var In=[[`day`,864e5],[`hour`,36e5],[`minute`,6e4],[`second`,1e3],[`millisecond`,1]];function Ln(e,t={}){if(!Number.isFinite(e))return``;let{locale:n,largest:r=2,unitDisplay:i=`short`}=t,a=Math.abs(Math.round(e)),o=[];for(let[t,s]of In){if(t===`millisecond`&&(o.length||Math.abs(e)>=1e3))break;let c=Math.floor(a/s);if(a-=c*s,c>0&&o.push(new Intl.NumberFormat(n,{style:`unit`,unit:t,unitDisplay:i}).format(c)),o.length===r)break}o.length===0&&o.push(new Intl.NumberFormat(n,{style:`unit`,unit:`second`,unitDisplay:i}).format(0));let s=new Intl.ListFormat(n,{style:i,type:`unit`}).format(o);return e<0?`-${s}`:s}var Rn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},zn=class extends C{constructor(){super(...arguments),this.value=``,this.position=`end`,this.tailLength=8,this.truncated=!1,this.ownTitle=null}static{this.styles=o`
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
  `}connectedCallback(){super.connectedCallback(),typeof ResizeObserver<`u`&&(this.resizeObserver=new ResizeObserver(()=>this.measure()),this.resizeObserver.observe(this))}disconnectedCallback(){super.disconnectedCallback(),this.resizeObserver?.disconnect()}updated(){this.resizeObserver&&this.measure()}measure(){let e=this.renderRoot.querySelector(`.start`),t=!!e&&e.scrollWidth>e.clientWidth;t!==this.truncated&&(this.truncated=t);let n=this.getAttribute(`title`);(n===null||n===this.ownTitle)&&(t&&n!==this.value?(this.setAttribute(`title`,this.value),this.ownTitle=this.value):!t&&n!==null&&(this.removeAttribute(`title`),this.ownTitle=null))}render(){let e=[...this.value],t=this.position===`middle`&&e.length>this.tailLength&&this.tailLength>0,n=t?e.slice(0,-this.tailLength).join(``):this.value,r=t?e.slice(-this.tailLength).join(``):``;return b`<span part="start" class="start">${n}</span>${t?b`<span part="tail" class="tail">${r}</span>`:S}`}};Rn([T()],zn.prototype,`value`,void 0),Rn([T({reflect:!0})],zn.prototype,`position`,void 0),Rn([T({type:Number,attribute:`tail-length`})],zn.prototype,`tailLength`,void 0),Rn([T({type:Boolean,reflect:!0})],zn.prototype,`truncated`,void 0),zn=Rn([w(`lit-material-truncate`)],zn);var Bn=class extends Mt{constructor(e){if(super(e),this.it=S,e.type!==At.CHILD)throw Error(this.constructor.directiveName+`() can only be used in child bindings`)}render(e){if(e===S||e==null)return this._t=void 0,this.it=e;if(e===Ee)return e;if(typeof e!=`string`)throw Error(this.constructor.directiveName+`() called with a non-string value`);if(e===this.it)return this._t;this.it=e;let t=[e];return t.raw=t,this._t={_$litType$:this.constructor.resultType,strings:t,values:[]}}};Bn.directiveName=`unsafeHTML`,Bn.resultType=1;var Vn=class extends Bn{};Vn.directiveName=`unsafeSVG`,Vn.resultType=2;var Hn=jt(Vn),Un=new Map,Wn=new EventTarget,Gn=new Set,Kn=`icons-changed`,qn=Wn;function Jn(e){let t=e.replace(/^\s*(<\?xml[^>]*>\s*)?(<!--[\s\S]*?-->\s*)*/,``).trim(),n=/^<svg\b[^>]*>/i.exec(t);if(!n)return``;let r=n[0].replace(/\s(width|height|aria-hidden|focusable)\s*=\s*("[^"]*"|'[^']*')/gi,``).replace(/\s*\/?>$/,``);return/\sfill\s*=/i.test(r)||(r+=` fill="currentColor"`),r+=` aria-hidden="true" focusable="false">`,r+t.slice(n[0].length)}function Yn(e,t={}){for(let[n,r]of Object.entries(e)){let e=t.prefix?`${t.prefix}:${n}`:n,i=Jn(r);i?(Un.set(e,i),Gn.delete(e)):console.warn(`lit-material-icon: "${e}" is not an <svg>; skipped.`)}Wn.dispatchEvent(new Event(Kn))}function Xn(e){return Un.get(e)}function Zn(e){Gn.has(e)||(Gn.add(e),console.warn(`lit-material-icon: no icon registered as "${e}".`))}var Qn=o`
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
`,$n=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},er=class extends C{constructor(){super(...arguments),this.size=`medium`,this.color=`inherit`,this.label=``,this.name=``,this.onIconsChanged=()=>{this.name&&this.requestUpdate()}}static{this.styles=Qn}connectedCallback(){super.connectedCallback(),qn.addEventListener(Kn,this.onIconsChanged)}disconnectedCallback(){super.disconnectedCallback(),qn.removeEventListener(Kn,this.onIconsChanged)}willUpdate(){this.label?(this.setAttribute(`role`,`img`),this.setAttribute(`aria-label`,this.label),this.removeAttribute(`aria-hidden`)):(this.removeAttribute(`role`),this.removeAttribute(`aria-label`),this.setAttribute(`aria-hidden`,`true`))}render(){if(!this.name)return b`<slot part="icon"></slot>`;let e=Xn(this.name);if(e===void 0){let e=this.name;return queueMicrotask(()=>Xn(e)===void 0&&Zn(e)),b``}return Hn(e)}};$n([T({reflect:!0})],er.prototype,`size`,void 0),$n([T({reflect:!0})],er.prototype,`color`,void 0),$n([T()],er.prototype,`label`,void 0),$n([T({reflect:!0})],er.prototype,`name`,void 0),er=$n([w(`lit-material-icon`)],er);function Q(e,t={}){return b`<lit-material-icon
    name=${e}
    label=${t.label??S}
    size=${t.size??S}
    slot=${t.slot??S}
  ></lit-material-icon>`}Yn({gauge:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M207.06,72.67A111.24,111.24,0,0,0,128,40h-.4C66.07,40.21,16,91,16,153.13V176a16,16,0,0,0,16,16H224a16,16,0,0,0,16-16V152A111.25,111.25,0,0,0,207.06,72.67ZM224,176H119.71l54.76-75.3a8,8,0,0,0-12.94-9.42L99.92,176H32V153.13c0-3.08.15-6.12.43-9.13H56a8,8,0,0,0,0-16H35.27c10.32-38.86,44-68.24,84.73-71.66V80a8,8,0,0,0,16,0V56.33A96.14,96.14,0,0,1,221,128H200a8,8,0,0,0,0,16h23.67c.21,2.65.33,5.31.33,8Z"/></svg>`,"chart-line":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M232,208a8,8,0,0,1-8,8H32a8,8,0,0,1-8-8V48a8,8,0,0,1,16,0v94.37L90.73,98a8,8,0,0,1,10.07-.38l58.81,44.11L218.73,90a8,8,0,1,1,10.54,12l-64,56a8,8,0,0,1-10.07.38L96.39,114.29,40,163.63V200H224A8,8,0,0,1,232,208Z"/></svg>`,"tree-structure":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M160,112h48a16,16,0,0,0,16-16V48a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16V64H128a24,24,0,0,0-24,24v32H72v-8A16,16,0,0,0,56,96H24A16,16,0,0,0,8,112v32a16,16,0,0,0,16,16H56a16,16,0,0,0,16-16v-8h32v32a24,24,0,0,0,24,24h16v16a16,16,0,0,0,16,16h48a16,16,0,0,0,16-16V160a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16v16H128a8,8,0,0,1-8-8V88a8,8,0,0,1,8-8h16V96A16,16,0,0,0,160,112ZM56,144H24V112H56v32Zm104,16h48v48H160Zm0-112h48V96H160Z"/></svg>`,certificate:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,136a8,8,0,0,1-8,8H72a8,8,0,0,1,0-16h48A8,8,0,0,1,128,136Zm-8-40H72a8,8,0,0,0,0,16h48a8,8,0,0,0,0-16Zm112,65.47V224A8,8,0,0,1,220,231l-24-13.74L172,231A8,8,0,0,1,160,224V200H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216a16,16,0,0,1,16,16V86.53a51.88,51.88,0,0,1,0,74.94ZM160,184V161.47A52,52,0,0,1,216,76V56H40V184Zm56-12a51.88,51.88,0,0,1-40,0v38.22l16-9.16a8,8,0,0,1,7.94,0l16,9.16Zm16-48a36,36,0,1,0-36,36A36,36,0,0,0,232,124Z"/></svg>`,"sliders-horizontal":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M40,88H73a32,32,0,0,0,62,0h81a8,8,0,0,0,0-16H135a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16Zm64-24A16,16,0,1,1,88,80,16,16,0,0,1,104,64ZM216,168H199a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16h97a32,32,0,0,0,62,0h17a8,8,0,0,0,0-16Zm-48,24a16,16,0,1,1,16-16A16,16,0,0,1,168,192Z"/></svg>`,"list-bullets":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M80,64a8,8,0,0,1,8-8H216a8,8,0,0,1,0,16H88A8,8,0,0,1,80,64Zm136,56H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16Zm0,64H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16ZM44,52A12,12,0,1,0,56,64,12,12,0,0,0,44,52Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,116Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,180Z"/></svg>`,"terminal-window":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,128a8,8,0,0,1-3,6.25l-40,32a8,8,0,1,1-10-12.5L107.19,128,75,102.25a8,8,0,1,1,10-12.5l40,32A8,8,0,0,1,128,128Zm48,24H136a8,8,0,0,0,0,16h40a8,8,0,0,0,0-16Zm56-96V200a16,16,0,0,1-16,16H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216A16,16,0,0,1,232,56ZM216,200V56H40V200H216Z"/></svg>`,"sign-out":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M120,216a8,8,0,0,1-8,8H48a8,8,0,0,1-8-8V40a8,8,0,0,1,8-8h64a8,8,0,0,1,0,16H56V208h56A8,8,0,0,1,120,216Zm109.66-93.66-40-40a8,8,0,0,0-11.32,11.32L204.69,120H112a8,8,0,0,0,0,16h92.69l-26.35,26.34a8,8,0,0,0,11.32,11.32l40-40A8,8,0,0,0,229.66,122.34Z"/></svg>`,"check-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M173.66,98.34a8,8,0,0,1,0,11.32l-56,56a8,8,0,0,1-11.32,0l-24-24a8,8,0,0,1,11.32-11.32L112,148.69l50.34-50.35A8,8,0,0,1,173.66,98.34ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,"warning-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24Zm0,192a88,88,0,1,1,88-88A88.1,88.1,0,0,1,128,216Zm-8-80V80a8,8,0,0,1,16,0v56a8,8,0,0,1-16,0Zm20,36a12,12,0,1,1-12-12A12,12,0,0,1,140,172Z"/></svg>`,"x-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M165.66,101.66,139.31,128l26.35,26.34a8,8,0,0,1-11.32,11.32L128,139.31l-26.34,26.35a8,8,0,0,1-11.32-11.32L116.69,128,90.34,101.66a8,8,0,0,1,11.32-11.32L128,116.69l26.34-26.35a8,8,0,0,1,11.32,11.32ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,pause:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M200,32H160a16,16,0,0,0-16,16V208a16,16,0,0,0,16,16h40a16,16,0,0,0,16-16V48A16,16,0,0,0,200,32Zm0,176H160V48h40ZM96,32H56A16,16,0,0,0,40,48V208a16,16,0,0,0,16,16H96a16,16,0,0,0,16-16V48A16,16,0,0,0,96,32Zm0,176H56V48H96Z"/></svg>`,play:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M232.4,114.49,88.32,26.35a16,16,0,0,0-16.2-.3A15.86,15.86,0,0,0,64,39.87V216.13A15.94,15.94,0,0,0,80,232a16.07,16.07,0,0,0,8.36-2.35L232.4,141.51a15.81,15.81,0,0,0,0-27ZM80,215.94V40l143.83,88Z"/></svg>`,"arrow-clockwise":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M240,56v48a8,8,0,0,1-8,8H184a8,8,0,0,1,0-16H211.4L184.81,71.64l-.25-.24a80,80,0,1,0-1.67,114.78,8,8,0,0,1,11,11.63A95.44,95.44,0,0,1,128,224h-1.32A96,96,0,1,1,195.75,60L224,85.8V56a8,8,0,1,1,16,0Z"/></svg>`,trash:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M216,48H176V40a24,24,0,0,0-24-24H104A24,24,0,0,0,80,40v8H40a8,8,0,0,0,0,16h8V208a16,16,0,0,0,16,16H192a16,16,0,0,0,16-16V64h8a8,8,0,0,0,0-16ZM96,40a8,8,0,0,1,8-8h48a8,8,0,0,1,8,8v8H96Zm96,168H64V64H192ZM112,104v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Zm48,0v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Z"/></svg>`,plus:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M224,128a8,8,0,0,1-8,8H136v80a8,8,0,0,1-16,0V136H40a8,8,0,0,1,0-16h80V40a8,8,0,0,1,16,0v80h80A8,8,0,0,1,224,128Z"/></svg>`,"lock-key":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,112a28,28,0,0,0-8,54.83V184a8,8,0,0,0,16,0V166.83A28,28,0,0,0,128,112Zm0,40a12,12,0,1,1,12-12A12,12,0,0,1,128,152Zm80-72H176V56a48,48,0,0,0-96,0V80H48A16,16,0,0,0,32,96V208a16,16,0,0,0,16,16H208a16,16,0,0,0,16-16V96A16,16,0,0,0,208,80ZM96,56a32,32,0,0,1,64,0V80H96ZM208,208H48V96H208V208Z"/></svg>`,"clipboard-text":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M168,152a8,8,0,0,1-8,8H96a8,8,0,0,1,0-16h64A8,8,0,0,1,168,152Zm-8-40H96a8,8,0,0,0,0,16h64a8,8,0,0,0,0-16Zm56-64V216a16,16,0,0,1-16,16H56a16,16,0,0,1-16-16V48A16,16,0,0,1,56,32H92.26a47.92,47.92,0,0,1,71.48,0H200A16,16,0,0,1,216,48ZM96,64h64a32,32,0,0,0-64,0ZM200,48H173.25A47.93,47.93,0,0,1,176,64v8a8,8,0,0,1-8,8H88a8,8,0,0,1-8-8V64a47.93,47.93,0,0,1,2.75-16H56V216H200Z"/></svg>`,"user-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24ZM74.08,197.5a64,64,0,0,1,107.84,0,87.83,87.83,0,0,1-107.84,0ZM96,120a32,32,0,1,1,32,32A32,32,0,0,1,96,120Zm97.76,66.41a79.66,79.66,0,0,0-36.06-28.75,48,48,0,1,0-59.4,0,79.66,79.66,0,0,0-36.06,28.75,88,88,0,1,1,131.52,0Z"/></svg>`});var tr=[{id:`overview`,label:`Overview`,icon:`gauge`},{id:`traffic`,label:`Traffic`,icon:`chart-line`},{id:`upstreams`,label:`Upstreams`,icon:`tree-structure`},{id:`certificates`,label:`Certificates`,icon:`certificate`},{id:`controls`,label:`Controls`,icon:`sliders-horizontal`},{id:`config`,label:`Config`,icon:`list-bullets`},{id:`logs`,label:`Logs`,icon:`terminal-window`},{id:`audit`,label:`Audit`,icon:`clipboard-text`}];function nr(){let e=location.hash.replace(/^#\/?/,``);return tr.some(t=>t.id===e)?e:`overview`}var rr=e=>Ln(e*1e3,{unitDisplay:`narrow`}),ir=b`<svg class="brand-mark" viewBox="0 0 32 32" aria-hidden="true"><rect width="32" height="32" rx="7" fill="#D2551E"/><g transform="translate(4 3.5)"><rect x="2" y="5" width="20" height="4" rx="1.5" fill="#16120F"/><rect x="2" y="10.5" width="20" height="4" rx="1.5" fill="#16120F"/><path d="M3.5 16H15L14 17L15 18L14 19L15 20H3.5A1.5 1.5 0 0 1 2 18.5V17.5A1.5 1.5 0 0 1 3.5 16Z" fill="#F2EBE3"/><rect x="16.5" y="16.5" width="2" height="2" fill="#F2EBE3"/><rect x="19.5" y="15" width="1.4" height="1.4" fill="#F2EBE3"/></g></svg>`,$=(e,t)=>b`<lit-material-status status=${e}>${t}</lit-material-status>`,ar=e=>new Date(e).toLocaleTimeString(),or=e=>/"status":5\d\d/.test(e)?`error`:/"status":4\d\d/.test(e)?`warning`:null,sr=(e,t)=>[{key:`url`,label:`Backend`,mono:!0},{key:`weight`,label:`Weight`,numeric:!0},{key:`healthy`,label:`Health`,render:e=>e.healthy?$(`ok`,`Healthy`):$(`error`,`Failing checks`)},{key:`breaker`,label:`Breaker`,render:e=>e.breaker===null?$(`neutral`,`None`):e.breaker===`closed`?$(`ok`,`Closed`):e.breaker===`half_open`?$(`warning`,`Probing`):$(`error`,`Open`)},{key:`rotation`,label:`In rotation`,render:n=>b`<div class="row">
      <lit-material-switch aria-label="${n.url} in rotation" .checked=${!n.drained} ?disabled=${e.busy===`drain:${n.url}`}
        @change=${r=>e.drain(t,n.url,!r.target.checked)}></lit-material-switch>
      ${n.drained?$(`warning`,`Drained`):S}
    </div>`}],cr=[{key:`name`,label:`Name`,mono:!0},{key:`expires`,label:`Expires`},{key:`days_left`,label:`Days left`,numeric:!0,render:e=>e.days_left<7?$(`error`,String(e.days_left)):e.days_left<21?$(`warning`,String(e.days_left)):e.days_left},{key:`source`,label:`Source`,render:e=>e.source===`on_demand`?`On demand`:`Configured`}],lr=class extends C{static properties={authed:{state:!0},view:{state:!0},loginError:{state:!0},data:{state:!0},error:{state:!0},samples:{state:!0},paused:{state:!0},filter:{state:!0},busy:{state:!0},trafficHours:{state:!0},trafficHost:{state:!0},me:{state:!0},sso:{state:!0}};constructor(){super(),this.authed=!1,this.view=nr(),this.data={},this.samples=[],this.paused=!1,this.filter=``,this.trafficHours=24,this.trafficHost=``,this.busy=``,this.lastTotal=null,this.me=null,this.sso=!1}createRenderRoot(){return this}connectedCallback(){super.connectedCallback(),this.onHash=()=>this.show(nr()),addEventListener(`hashchange`,this.onHash),this.check({quiet:!kn()}),Z(`/auth`).then(e=>this.sso=!!e?.oidc).catch(()=>{})}disconnectedCallback(){super.disconnectedCallback(),removeEventListener(`hashchange`,this.onHash),this.stop()}async check({quiet:e=!1}={}){try{let[e,t]=await Promise.all([Z(`/whoami`),Z(`/status`)]);this.me=e,this.data={...this.data,status:t},this.authed=!0,this.loginError=``,this.show(this.view)}catch(t){if(this.authed=!1,e&&t instanceof X&&t.status===401)return;this.loginError=t instanceof X&&t.status===401?`That token isn't an admin token. Check RWS_ADMIN_TOKEN (or your token_env) on the server.`:t instanceof X&&t.status===403?t.message:`The admin API didn't answer: ${t.message}`,t instanceof X&&t.status===401&&An(``)}}login(e){e.preventDefault();let t=(this.querySelector(`#token`)?.value??``).trim();t?(An(t),this.check()):this.loginError=`Paste the admin token.`}logout(){let e=this.me?.via===`oidc`;An(``),this.stop(),this.authed=!1,this.data={},this.me=null,e&&location.assign(`/admin/auth/logout`)}async call(e,t){try{let n=await Z(e,t);return this.error=``,n}catch(e){throw e instanceof X&&e.status===401?(this.logout(),this.loginError=`The server no longer accepts this token. Sign in again.`):this.error=e.message,e}}toast(e){this.querySelector(`#toast`)?.show(e)}show(e){this.stop(),this.view=e,location.hash!==`#/${e}`&&history.replaceState(null,``,`#/${e}`),this.error=``;let t={overview:2e3,upstreams:3e3,certificates:3e4,controls:5e3,traffic:1e4,audit:1e4}[e];this.load(e),t&&(this.timer=setInterval(()=>this.load(e),t)),e===`logs`&&this.updateComplete.then(()=>this.follow()),queueMicrotask(()=>this.querySelector(`main`)?.focus({preventScroll:!0}))}stop(){clearInterval(this.timer),this.timer=void 0,this.abort?.abort(),this.abort=void 0}async load(e){let t=async(e,t)=>{try{let n=await this.call(t);this.data={...this.data,[e]:n}}catch{}};if(e===`overview`){await Promise.all([t(`status`,`/status`),t(`metrics`,`/metrics`)]);let e=this.data.metrics?.requests_total;if(typeof e==`number`){let t=Date.now();if(this.lastTotal){let n=Math.max(0,(e-this.lastTotal.total)/((t-this.lastTotal.at)/1e3));this.samples=[...this.samples,{x:t,y:Math.round(n*100)/100}].slice(-60)}this.lastTotal={total:e,at:t}}}else e===`upstreams`?await t(`upstreams`,`/upstreams`):e===`certificates`?await t(`certificates`,`/certificates`):e===`controls`?await Promise.all([t(`maintenance`,`/maintenance`),t(`blocklist`,`/blocklist`),t(`flags`,`/flags`),t(`rateLimit`,`/rate-limit`),t(`caches`,`/caches`),this.loadAuthz()]):e===`traffic`?await this.loadTraffic():e===`config`?await t(`config`,`/config`):e===`audit`&&await t(`audit`,`/audit?n=200`)}async follow(){this.abort=new AbortController;let e=this.querySelector(`lit-material-log-viewer`);if(e){e.highlight=or;try{e.lines=await this.call(`/logs?n=200`),await jn(t=>e.appendLines(t),this.abort.signal)}catch(e){e.name!==`AbortError`&&!this.error&&(this.error=`The live log stopped: ${e.message}`)}}}async act(e,t,n,r){this.busy=e;try{let e=await this.call(t,n);r&&this.toast(r(e)),await this.load(this.view)}catch{}finally{this.busy=``}}drain(e,t,n){return this.act(`drain:${t}`,`/upstreams/${encodeURIComponent(e)}/drain`,{method:`PUT`,body:{backend:t,drained:n}},()=>n?`${t} drained: it gets no new requests`:`${t} is back in rotation`)}reload(){return this.act(`reload`,`/reload`,{method:`POST`},e=>e.applied?`Configuration reloaded`:`Reload kept the running configuration: ${e.errors.join(`; `)}`)}block(e){e.preventDefault();let t=this.querySelector(`#block-ip`),n=(t?.value??``).trim();n&&this.act(`block`,`/blocklist`,{method:`POST`,body:{ip:n}},e=>`${e.blocked} blocked`).then(()=>{!this.error&&t&&(t.value=``)})}async loadTraffic(){let e=this.trafficHost?`&host=${encodeURIComponent(this.trafficHost)}`:``;try{let[t,n]=await Promise.all([Z(`/traffic?hours=${this.trafficHours}${e}`),Z(`/traffic/live?${e.slice(1)}`).catch(()=>null)]);this.data={...this.data,traffic:t,trafficLive:n}}catch(e){e instanceof X&&e.status===404&&(this.data={...this.data,traffic:null})}}setTraffic(e,t){this.trafficHours=e,this.trafficHost=t,this.data={...this.data,traffic:void 0},this.loadTraffic()}async loadAuthz(){try{let[e,t]=await Promise.all([Z(`/authz`),Z(`/authz/tuples?limit=200`)]);this.data={...this.data,authz:{schema:e,tuples:t.tuples}}}catch(e){e instanceof X&&e.status===404&&(this.data={...this.data,authz:null})}}addTuple(e){e.preventDefault();let t=this.querySelector(`#authz-tuple`),n=(t?.value??``).trim();n&&this.act(`authz-add`,`/authz/tuples`,{method:`POST`,body:{tuple:n}},e=>e.written?`${e.tuple} added`:`${e.tuple} was already there`).then(()=>{!this.error&&t&&(t.value=``)})}async checkAccess(e){e.preventDefault();let t=e=>(this.querySelector(e)?.value??``).trim(),n={subject:t(`#authz-subject`),relation:t(`#authz-relation`),object:t(`#authz-object`)};if(n.subject&&n.relation&&n.object)try{let e=await this.call(`/authz/check`,{method:`POST`,body:n});this.data={...this.data,authzCheck:{...n,...e}}}catch{}}purge(e){let t=(this.querySelector(`#purge-path`)?.value??``).trim(),n=t?t.endsWith(`*`)?`?prefix=${encodeURIComponent(t.slice(0,-1))}`:`?path=${encodeURIComponent(t)}`:``;return this.act(`purge:${e}`,`/caches/${encodeURIComponent(e)}${n}`,{method:`DELETE`},e=>`${e.purged} ${e.purged===1?`entry`:`entries`} purged from ${e.name}`)}render(){if(!this.authed)return this.renderLogin();let e=this.data.status,t=tr.map(e=>({label:e.label,icon:Q(e.icon)}));return b`
      <div class="shell">
        <header class="top">
          <span class="brand">${ir}<span class="brand-text">rws admin</span></span>
          ${e?e.ready?$(`ok`,`Ready`):$(`error`,`Not ready`):S}
          <span class="spacer"></span>
          ${e?b`<span class="meta">${e.version} · ${e.listeners?.main?.address??``}</span>`:S}
          ${this.me?b`<span class="meta" title="Signed in ${this.me.via===`oidc`?`with single sign-on`:`with a token`}"><span class="icon">${Q(`user-circle`)}</span> ${this.me.user} · ${this.me.role}</span>`:S}
          <lit-material-button variant="text" @click=${()=>this.logout()}><span slot="icon" class="icon">${Q(`sign-out`)}</span>Sign out</lit-material-button>
        </header>
        <lit-material-adaptive-navigation class="nav" label="Sections" .destinations=${t}
          .selected=${tr.findIndex(e=>e.id===this.view)}
          @change=${e=>this.show(tr[e.target.selected].id)}></lit-material-adaptive-navigation>
        <main id="main" tabindex="-1">
          ${this.error?b`<lit-material-alert variant="error"><span slot="title">Something failed</span>${this.error}</lit-material-alert>`:S}
          ${this.renderView()}
        </main>
      </div>
      <lit-material-snackbar id="toast"></lit-material-snackbar>
    `}renderLogin(){return b`
      <div class="login">
        <section class="login-card tile" aria-labelledby="login-title">
          <h1 id="login-title"><span class="icon">${Q(`lock-key`)}</span>Sign in to rws admin</h1>
          ${this.sso?b`<lit-material-button href="/admin/auth/login" style="width: 100%">Sign in with single sign-on</lit-material-button>
            <p class="lede" style="text-align: center">or with a token</p>`:S}
          <p>Paste an admin token: the value of <code>RWS_ADMIN_TOKEN</code> (or the variable your <code>[admin] token_env</code> names), or your own from <code>[admin] users</code>. It stays in this tab only.</p>
          <form @submit=${e=>this.login(e)} novalidate>
            <lit-material-text-field id="token" variant="outlined" label="Admin token" type="password" autocomplete="current-password" required
              ?error=${!!this.loginError} error-text=${this.loginError??``}></lit-material-text-field>
            <lit-material-button type="submit">Sign in</lit-material-button>
          </form>
        </section>
      </div>
    `}renderView(){switch(this.view){case`upstreams`:return this.renderUpstreams();case`certificates`:return this.renderCertificates();case`controls`:return this.renderControls();case`traffic`:return this.renderTraffic();case`config`:return this.renderConfig();case`logs`:return this.renderLogs();case`audit`:return this.renderAudit();default:return this.renderOverview()}}skeleton(e=3){return b`<div class="grid" aria-busy="true">${Array.from({length:e},()=>b`<div class="tile"><lit-material-skeleton width="40%"></lit-material-skeleton><lit-material-skeleton variant="rounded" height="32px"></lit-material-skeleton></div>`)}</div>`}renderOverview(){let e=this.data.status,t=this.data.metrics;if(!e||!t)return b`<h1>Overview</h1>${this.skeleton(4)}`;let n=t.requests_total?(t.errors_total/t.requests_total*100).toFixed(2):`0.00`,r=this.samples.map(e=>e.y);return b`
      <h1>Overview</h1>
      <p class="lede">Up ${rr(e.uptime_secs)}, process ${e.pid}. Counters refresh every 2 seconds.</p>
      ${(e.disk??[]).filter(e=>e.low).map(e=>b`<lit-material-alert variant="warning" style="margin-bottom: var(--space-200)">
        <span slot="title">Disk almost full</span>
        ${e.dir} has ${Fn(e.free_bytes)} free of ${Fn(e.total_bytes)}: rws has stopped writing the access log and disk cache there until space is freed.</lit-material-alert>`)}
      <div class="grid">
        <lit-material-stat label="Requests served" value=${t.requests_total.toLocaleString()}>
          ${r.length>1?b`<lit-material-sparkline slot="caption" label="Requests per second" .values=${r}></lit-material-sparkline>`:S}
        </lit-material-stat>
        <lit-material-stat label="Errors" value=${t.errors_total.toLocaleString()} caption="${n}% of requests"></lit-material-stat>
        <lit-material-stat label="Open connections" value=${String(t.active_connections)}></lit-material-stat>
        <lit-material-stat label="Queued for a worker" value=${String(t.thread_pool_queued)}></lit-material-stat>
      </div>
      <section class="section" aria-labelledby="rate-title">
        <h2 id="rate-title">Requests per second</h2>
        ${this.samples.length<2?b`<lit-material-empty-state size="compact" headline="Collecting samples">The chart fills in as this page stays open.</lit-material-empty-state>`:b`<lit-material-line-chart label="Requests per second, last ${Math.round((this.samples.at(-1).x-this.samples[0].x)/1e3)} seconds"
              unit="req/s" height="200" .formatX=${ar} .series=${[{name:`Requests per second`,points:this.samples}]}></lit-material-line-chart>`}
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
    `}renderUpstreams(){let e=this.data.upstreams;return e?b`
      <h1>Upstreams</h1>
      <p class="lede">Drain a backend before deploying it: new requests go elsewhere, requests already on it finish, and its health checks keep running.</p>
      ${e.length===0?b`<lit-material-empty-state size="compact" headline="No upstreams configured">Upstreams come from <code>[[upstream]]</code> sections in <code>rws.config.toml</code>. Add one, then reload from Controls.</lit-material-empty-state>`:e.map(e=>b`
          <section class="section" aria-labelledby="up-${e.name}">
            <h2 id="up-${e.name}">${e.name}</h2>
            <lit-material-table caption="Backends of ${e.name}" caption-hidden .columns=${sr(this,e.name)} .rows=${e.backends}></lit-material-table>
          </section>`)}
    `:b`<h1>Upstreams</h1>${this.skeleton(2)}`}renderCertificates(){let e=this.data.certificates;if(!e)return b`<h1>Certificates</h1>${this.skeleton(2)}`;let t=e.acme;return b`
      <h1>Certificates</h1>
      <p class="lede">Every certificate this server presents, soonest expiry first.</p>
      ${e.certificates.length===0?b`<lit-material-empty-state size="compact" headline="No TLS certificates">This server speaks plain HTTP. Set <code>tls_cert_file</code> and <code>tls_key_file</code>, or <code>[acme] domains</code>, to serve HTTPS.</lit-material-empty-state>`:b`<lit-material-table caption="Certificates" caption-hidden .columns=${cr} .rows=${e.certificates}></lit-material-table>`}
      <section class="section" aria-labelledby="acme-title">
        <h2 id="acme-title">ACME</h2>
        ${t.running?b`<p class="lede">Renewal runs automatically. ${t.last_check?b`Last check ${new Date(t.last_check.at*1e3).toLocaleString()}: ${t.last_check.outcome}.`:`No check has finished yet.`}</p>
              <lit-material-button variant="outlined" ?disabled=${this.busy===`renew`}
                @click=${()=>this.act(`renew`,`/certificates/renew`,{method:`POST`},()=>`Renewal requested: the new certificate is swapped in when it's issued`)}>
                <span slot="icon" class="icon">${Q(`arrow-clockwise`)}</span>Renew now</lit-material-button>`:b`<lit-material-empty-state size="compact" headline="ACME is off">Set <code>[acme] domains</code> to have rws obtain and renew certificates itself.</lit-material-empty-state>`}
      </section>
    `}renderControls(){let e=this.data;return!e.maintenance||!e.blocklist||!e.flags?b`<h1>Controls</h1>${this.skeleton(3)}`:b`
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
        ${e.blocklist.length===0?b`<p class="lede" style="margin-top: var(--space-200)">Nobody is blocked.</p>`:b`<lit-material-table style="margin-top: var(--space-200)" caption="Blocked addresses" caption-hidden density="compact"
              .columns=${[{key:`ip`,label:`Address`,mono:!0},{key:`action`,label:`Action`,hiddenLabel:!0,render:e=>b`<lit-material-button variant="text" @click=${()=>this.act(`unblock:${e.ip}`,`/blocklist/${encodeURIComponent(e.ip)}`,{method:`DELETE`},()=>`${e.ip} unblocked`)}>Unblock</lit-material-button>`}]} .rows=${e.blocklist.map(e=>({ip:e}))}></lit-material-table>`}
      </section>

      ${e.authz?this.renderAuthz(e.authz,e.authzCheck):S}

      <section class="section" aria-labelledby="caches-title">
        <h2 id="caches-title">Response caches</h2>
        ${!e.caches||e.caches.length===0?b`<lit-material-empty-state size="compact" headline="No response cache">Add <code>[route.middleware.cache]</code> to a route; its cache appears here to purge.</lit-material-empty-state>`:b`<lit-material-text-field id="purge-path" variant="outlined" label="Path to purge" supporting-text="Empty purges everything; /products/* purges a prefix"></lit-material-text-field>
              <lit-material-table style="margin-top: var(--space-200)" caption="Response caches" caption-hidden density="compact" .columns=${[{key:`name`,label:`Cache`,mono:!0},{key:`kind`,label:`Store`},{key:`entries`,label:`Entries`,numeric:!0},{key:`bytes`,label:`On disk`,numeric:!0,render:e=>e.bytes===null?`—`:Fn(e.bytes)},{key:`hits`,label:`Hits / misses`,numeric:!0,render:e=>`${e.hits} / ${e.misses}`},{key:`action`,label:`Action`,hiddenLabel:!0,render:e=>b`<lit-material-button variant="text" ?disabled=${this.busy===`purge:${e.name}`} @click=${()=>this.purge(e.name)}><span slot="icon" class="icon">${Q(`trash`)}</span>Purge</lit-material-button>`}]} .rows=${e.caches}></lit-material-table>`}
      </section>

      <section class="section" aria-labelledby="flags-title">
        <h2 id="flags-title">Feature flags</h2>
        ${e.flags.length===0?b`<lit-material-empty-state size="compact" headline="No feature flags">Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</lit-material-empty-state>`:b`<lit-material-table caption="Feature flags" caption-hidden density="compact" .columns=${[{key:`name`,label:`Flag`,mono:!0},{key:`enabled`,label:`Enabled`,render:e=>b`<lit-material-switch aria-label="${e.name}" .checked=${e.enabled}
          @change=${t=>this.act(`flag:${e.name}`,`/flags/${encodeURIComponent(e.name)}`,{method:`PUT`,body:{enabled:t.target.checked}},e=>`${e.name} ${e.enabled?`on`:`off`}`)}></lit-material-switch>`}]} .rows=${e.flags}></lit-material-table>`}
      </section>
    `}renderTraffic(){let e=this.data.traffic;if(e===null)return b`<h1>Traffic</h1><lit-material-empty-state headline="Traffic counting is off">It's on whenever the admin API is, unless <code>[traffic] enabled = false</code>.</lit-material-empty-state>`;if(!e)return b`<h1>Traffic</h1>${this.skeleton(4)}`;let t=[[24,`Day`],[168,`Week`],[720,`30 days`]],n={24:`day`,168:`week`,720:`30 days`}[e.hours]??`${e.hours} hours`,r=(e,t)=>t?`${e>=t?`+`:`−`}${Math.abs(Math.round((e-t)/t*100))}% vs the previous ${n}.`:``,i=[[`people`,`People`,`Browsers that loaded the page's styles or scripts`],[`ai`,`AI agents`,`AI crawlers, AI search and assistants fetching for someone`],[`bots`,`Bots`,`Search engines, SEO tools, monitors, previews, scanners and the like`]],a=i.map(([t,n])=>({name:n,points:e.series.map(e=>({x:e.t*1e3,y:e[t]}))})),o=e=>e.map(([e,t])=>({key:e,count:t})),s={key:`count`,label:`Requests`,numeric:!0,render:e=>e.count.toLocaleString()},c=e=>e.replaceAll(`-`,` `),l=(e,t,r,i)=>b`
      <section class="section" aria-label=${e}>
        <h2>${e}</h2>
        ${r.length===0?b`<p class="lede">Nothing in this ${n}.</p>`:b`<lit-material-table caption=${t} caption-hidden density="compact" .columns=${i} .rows=${r}></lit-material-table>`}
      </section>`;return b`
      <h1>Traffic</h1>
      ${this.renderTrafficLive(this.data.trafficLive)}
      <p class="lede">Who visited in the last ${n}. People are browsers that load the page's styles or scripts; AI agents and bots are known by name, published network or behaviour. No IP addresses or User-Agents are kept.</p>
      <div class="row" style="margin-bottom: var(--space-300)">
        ${t.map(([t,n])=>b`<lit-material-button variant=${e.hours===t?`tonal`:`text`} @click=${()=>this.setTraffic(t,this.trafficHost)}>${n}</lit-material-button>`)}
        ${e.hosts.length>1?b`<span style="width: var(--space-300)"></span>
              <lit-material-button variant=${e.host?`text`:`tonal`} @click=${()=>this.setTraffic(this.trafficHours,``)}>All sites</lit-material-button>
              ${e.hosts.map(t=>b`<lit-material-button variant=${e.host===t?`tonal`:`text`} @click=${()=>this.setTraffic(this.trafficHours,t)}>${t}</lit-material-button>`)}`:S}
      </div>
      <div class="grid">
        ${i.map(([t,n,i])=>b`
          <lit-material-stat label=${`${n}: visitors`} value=${e.totals[t].visitors.toLocaleString()}
            caption=${`${e.totals[t].pages.toLocaleString()} page views, ${e.totals[t].requests.toLocaleString()} requests. ${r(e.totals[t].requests,e.previous[t].requests)}`} title=${i}>
            <lit-material-sparkline slot="caption" label=${`${n}: requests per ${e.bucket}`} .values=${e.series.map(e=>e[t])}></lit-material-sparkline>
          </lit-material-stat>`)}
      </div>
      <section class="section" aria-label="Requests over time">
        <lit-material-line-chart label=${`Requests per ${e.bucket}`} unit="requests" height="240" .series=${a}
          .formatX=${t=>e.bucket===`hour`?new Date(t).toLocaleTimeString([],{hour:`2-digit`,minute:`2-digit`}):new Date(t).toLocaleDateString()}></lit-material-line-chart>
        ${e.host?b`<p class="lede">Visitors are counted for all sites together.</p>`:S}
      </section>
      ${this.renderTrafficSessions(e,n)}
      ${l(`Most read pages (people)`,`Pages people read`,o(e.people_pages),[{key:`key`,label:`Page`,mono:!0},{...s,label:`Views`}])}
      ${(e.entry_pages??[]).length?b`<div class="grid">
            ${l(`Entry pages`,`Pages sessions began on`,o(e.entry_pages),[{key:`key`,label:`Page`,mono:!0},{...s,label:`Sessions`}])}
            ${l(`Exit pages`,`Pages sessions ended on`,o(e.exit_pages??[]),[{key:`key`,label:`Page`,mono:!0},{...s,label:`Sessions`}])}
          </div>`:S}
      ${l(`AI agents`,`AI agents`,e.ai_agents,[{key:`name`,label:`Agent`},{key:`operator`,label:`Operator`},{key:`kind`,label:`Kind`,render:e=>c(e.kind)},{key:`verified`,label:`Network`,render:e=>e.verified?$(`ok`,`Verified`):$(`neutral`,`Unverified`)},{key:`requests`,label:`Requests`,numeric:!0,render:e=>e.requests.toLocaleString()}])}
      ${l(`Pages AI agents read`,`Pages AI agents read`,o(e.ai_pages),[{key:`key`,label:`Page`,mono:!0},s])}
      ${l(`Bots`,`Bots by kind`,e.bots,[{key:`kind`,label:`Kind`,render:e=>c(e.kind)},{key:`name`,label:`Name`,render:e=>e.name||`—`},{key:`requests`,label:`Requests`,numeric:!0,render:e=>e.requests.toLocaleString()}])}
      ${this.renderTrafficSources(e,l,o,s)}
      ${this.renderTrafficAudience(e,l)}
      ${l(`Not found`,`Paths answered 404`,o(e.not_found),[{key:`key`,label:`Path`,mono:!0},s])}
    `}renderTrafficLive(e){if(!e)return S;let t=e.pages.slice(0,5);return b`
      <section class="section" aria-label="Right now">
        <div class="grid">
          <lit-material-stat label="Right now" value=${e.visitors.toLocaleString()}
            caption=${`${e.visitors===1?`person`:`people`} in the last 5 minutes, ${e.views_5m.toLocaleString()} page view${e.views_5m===1?``:`s`}.`}>
            <lit-material-sparkline slot="caption" label="Page views per minute, last 30 minutes" .values=${e.series.map(e=>e.views)}></lit-material-sparkline>
          </lit-material-stat>
          ${t.length?b`<lit-material-table caption="Pages being read now" density="compact"
                .columns=${[{key:`key`,label:`Being read now`,mono:!0},{key:`count`,label:`Views`,numeric:!0}]}
                .rows=${t.map(([e,t])=>({key:e,count:t}))}></lit-material-table>`:S}
        </div>
      </section>`}renderTrafficSessions(e,t){let n=e.sessions;if(!n||!n.sessions)return S;let r=e.previous_sessions??{sessions:0,bounces:0,duration:0,pages:0},i=e=>e.sessions?e.bounces/e.sessions:0,a=e=>e.sessions?e.duration/e.sessions:0,o=e=>e.sessions?e.pages/e.sessions:0,s=(e,n,i=`%`)=>r.sessions?i===`pt`?` ${e>=n?`+`:`−`}${Math.abs(Math.round((e-n)*100))} points vs the previous ${t}.`:n?` ${e>=n?`+`:`−`}${Math.abs(Math.round((e-n)/n*100))}% vs the previous ${t}.`:``:``;return b`
      <section class="section" aria-label="Sessions">
        <div class="grid">
          ${[[`Sessions`,n.sessions.toLocaleString(),`Page views 30 minutes or less apart.${s(n.sessions,r.sessions)}`],[`Bounce rate`,`${Math.round(i(n)*100)}%`,`Sessions of one page view.${s(i(n),i(r),`pt`)}`],[`Session length`,n.duration?rr(Math.round(a(n))):`0s`,`On average, first page view to last.${s(a(n),a(r))}`],[`Pages per session`,o(n).toFixed(1),`On average.${s(o(n),o(r))}`]].map(([e,t,n])=>b`<lit-material-stat label=${e} value=${t} caption=${n}></lit-material-stat>`)}
        </div>
      </section>`}renderTrafficSources(e,t,n,r){let i={direct:`Direct`,search:`Search`,social:`Social`,ai:`AI assistants`,referral:`Other sites`,campaign:`Campaigns (UTM)`},a={...r,label:`Visits`},o=e=>n(e??[]);return b`
      <section class="section" aria-labelledby="traffic-sources">
        <h2 id="traffic-sources">Sources</h2>
        <p class="lede">How people arrived: a page view from outside the site. Moving between the site's own pages isn't counted here.</p>
        ${(e.channels??[]).length===0?b`<p class="lede">Nothing in this period.</p>`:b`<lit-material-bar-chart label="Visits by channel" sort="desc" .format=${e=>`${e.toLocaleString()} visit${e===1?``:`s`}`}
              .data=${e.channels.map(([e,t])=>({name:i[e]??e,value:t}))}></lit-material-bar-chart>`}
      </section>
      ${t(`Referrers (people)`,`Sites people came from`,n(e.referrers),[{key:`key`,label:`Site`,mono:!0},a])}
      ${(e.utm_source??[]).length||(e.utm_campaign??[]).length?b`<div class="grid">
            ${t(`Campaign sources`,`utm_source`,o(e.utm_source),[{key:`key`,label:`utm_source`,mono:!0},a])}
            ${t(`Campaign media`,`utm_medium`,o(e.utm_medium),[{key:`key`,label:`utm_medium`,mono:!0},a])}
            ${t(`Campaigns`,`utm_campaign`,o(e.utm_campaign),[{key:`key`,label:`utm_campaign`,mono:!0},a])}
          </div>`:S}`}renderTrafficAudience(e,t){let n=null;try{n=new Intl.DisplayNames([navigator.language||`en`],{type:`region`})}catch{}let r=e=>{if(e===`(unknown)`||e===`(other)`)return e===`(other)`?`Other`:`Unknown`;try{return n?.of(e)??e}catch{return e}},i=e=>e.reduce((e,[,t])=>e+t,0),a=e=>{let t=i(e)||1;return e.map(([e,n])=>({key:e,count:n,share:n/t}))},o=(e,t=e=>e.key)=>[{key:`key`,label:e,render:t},{key:`count`,label:`Visitors`,numeric:!0,render:e=>e.count.toLocaleString()},{key:`share`,label:`Share`,numeric:!0,render:e=>`${Math.round(e.share*100)}%`}],s={desktop:`Desktop`,mobile:`Phone`,tablet:`Tablet`};return b`
      <section class="section" aria-labelledby="traffic-audience">
        <h2 id="traffic-audience">Audience</h2>
        <p class="lede">People only, each counted once a day.
          ${e.geo_source?b`Countries from the built-in <a href="https://db-ip.com" target="_blank" rel="noopener">IP Geolocation by DB-IP</a> table (${e.geo_source}, CC BY 4.0), looked up on this server.`:b`No country table is loaded (built without the <code>geo</code> feature).`}</p>
        ${(e.devices??[]).length?b`<lit-material-bar-chart label="Visitors by device" sort="desc" .format=${e=>`${e.toLocaleString()} visitor${e===1?``:`s`}`}
              .data=${e.devices.map(([e,t])=>({name:s[e]??e,value:t}))}></lit-material-bar-chart>`:S}
      </section>
      ${t(`Countries`,`Visitors by country`,a(e.countries??[]),o(`Country`,e=>b`${r(e.key)} <span class="lede">${e.key.length===2?e.key:``}</span>`))}
      <div class="grid">
        ${t(`Browsers`,`Visitors by browser`,a(e.browsers??[]),o(`Browser`))}
        ${t(`Operating systems`,`Visitors by operating system`,a(e.os??[]),o(`System`))}
      </div>`}renderAuthz(e,t){return b`
      <section class="section" aria-labelledby="authz-title">
        <h2 id="authz-title">Authorization</h2>
        <p class="lede">Relationship tuples, object#relation@subject. Schema: ${Object.entries(e.schema.types).filter(([,e])=>e.length).map(([e,t])=>`${e}: ${t.join(`, `)}`).join(` · `)}.</p>
        <form class="row" @submit=${e=>this.addTuple(e)} novalidate>
          <lit-material-text-field id="authz-tuple" variant="outlined" label="Tuple" supporting-text="e.g. doc:42#viewer@user:alice"></lit-material-text-field>
          <lit-material-button type="submit" ?disabled=${this.busy===`authz-add`}><span slot="icon" class="icon">${Q(`plus`)}</span>Add</lit-material-button>
        </form>
        ${e.tuples.length===0?b`<p class="lede" style="margin-top: var(--space-200)">No tuples yet.</p>`:b`<lit-material-table style="margin-top: var(--space-200)" caption="Relationship tuples" caption-hidden density="compact"
              .columns=${[{key:`tuple`,label:`Tuple`,mono:!0},{key:`action`,label:`Action`,hiddenLabel:!0,render:e=>b`<lit-material-button variant="text" ?disabled=${this.busy===`authz-del:${e.tuple}`}
          @click=${()=>this.act(`authz-del:${e.tuple}`,`/authz/tuples?tuple=${encodeURIComponent(e.tuple)}`,{method:`DELETE`},()=>`${e.tuple} removed`)}>Remove</lit-material-button>`}]} .rows=${e.tuples.map(e=>({tuple:e}))}></lit-material-table>`}
        <form class="row" style="margin-top: var(--space-300)" @submit=${e=>this.checkAccess(e)} novalidate>
          <lit-material-text-field id="authz-subject" variant="outlined" label="Subject" supporting-text="user:alice"></lit-material-text-field>
          <lit-material-text-field id="authz-relation" variant="outlined" label="Relation" supporting-text="viewer"></lit-material-text-field>
          <lit-material-text-field id="authz-object" variant="outlined" label="Object" supporting-text="doc:42"></lit-material-text-field>
          <lit-material-button type="submit" variant="tonal">Check</lit-material-button>
        </form>
        ${t?b`<p class="lede" id="authz-result" style="margin-top: var(--space-200)">
              ${t.allowed?$(`ok`,`Allowed`):$(`error`,`Denied`)}
              ${t.subject} ${t.relation} ${t.object}${t.allowed?b` — via <code>${t.path.join(` → `)}</code>`:S}</p>`:S}
      </section>
    `}renderConfig(){let e=this.data.config;if(!e)return b`<h1>Config</h1>${this.skeleton(2)}`;let t=this.filter.toLowerCase(),n=Object.entries(e).filter(([e,n])=>!t||e.toLowerCase().includes(t)||n.toLowerCase().includes(t)).map(([e,t])=>({setting:e,value:t}));return b`
      <h1>Config</h1>
      <p class="lede">Every setting in effect. Secrets read (hidden). Change them in rws.config.toml or the environment, then reload from Controls.</p>
      <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${e=>this.filter=e.target.value}></lit-material-text-field>
      ${n.length===0?b`<lit-material-empty-state size="compact" headline="No setting matches “${this.filter}”">Clear the filter to see all ${Object.keys(e).length}.</lit-material-empty-state>`:b`<lit-material-table caption="Settings in effect" caption-hidden density="compact" .columns=${[{key:`setting`,label:`Setting`,mono:!0},{key:`value`,label:`Value`,mono:!0,render:e=>e.value===``?$(`neutral`,`empty`):b`<lit-material-truncate position="middle" style="max-width: min(60ch, 50vw)" .value=${e.value}></lit-material-truncate>`}]} .rows=${n}></lit-material-table>`}
    `}renderAudit(){let e=this.data.audit;return e?b`
      <h1>Audit</h1>
      <p class="lede">Every change made through the admin API, the UI or MCP tools, and every refused attempt, newest first. Kept in memory (the last 1000); <code>[admin] audit_log</code> also writes them to a file.</p>
      ${e.length===0?b`<lit-material-empty-state size="compact" headline="No changes yet">Changes appear here as people reload, drain, block or purge.</lit-material-empty-state>`:b`<lit-material-table caption="Recent changes" caption-hidden density="compact" .columns=${[{key:`time`,label:`When`,render:e=>new Date(e.at*1e3).toLocaleString()},{key:`user`,label:`Who`,render:e=>b`${e.user} <span class="meta">${e.role}${e.via===`oidc`?` · SSO`:e.via===`mcp`?` · MCP`:``}</span>`},{key:`change`,label:`Change`,mono:!0,render:e=>`${e.method===`TOOL`?`tool`:e.method} ${e.path.replace(`/admin/api/v1`,``)}`},{key:`detail`,label:`Details`,mono:!0,render:e=>e.detail?b`<lit-material-truncate style="max-width: min(40ch, 30vw)" .value=${e.detail}></lit-material-truncate>`:S},{key:`status`,label:`Result`,render:e=>e.status<300?$(`ok`,`Done`):e.status===401||e.status===403?$(`error`,`Refused`):$(`warning`,String(e.status))},{key:`address`,label:`From`,mono:!0}]} .rows=${e}></lit-material-table>`}
    `:b`<h1>Audit</h1>${this.skeleton(2)}`}renderLogs(){return b`
      <h1>Logs</h1>
      <div class="row" style="margin-bottom: var(--space-200)">
        <p class="lede" style="margin: 0; flex: 1">The access log as it's written, newest at the bottom. Scroll up to read; it stops following until you're back at the bottom.</p>
        <lit-material-button variant="outlined" @click=${()=>this.paused=!this.paused}>
          <span slot="icon" class="icon">${Q(this.paused?`play`:`pause`)}</span>${this.paused?`Resume`:`Pause`}</lit-material-button>
      </div>
      <lit-material-log-viewer label="Access log" max-lines="1000" ?paused=${this.paused}></lit-material-log-viewer>
    `}};customElements.define(`rws-admin`,lr);