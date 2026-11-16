var e=globalThis,t=e.ShadowRoot&&(e.ShadyCSS===void 0||e.ShadyCSS.nativeShadow)&&`adoptedStyleSheets`in Document.prototype&&`replace`in CSSStyleSheet.prototype,n=Symbol(),r=new WeakMap,i=class{constructor(e,t,r){if(this._$cssResult$=!0,r!==n)throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");this.cssText=e,this.t=t}get styleSheet(){let e=this.o,n=this.t;if(t&&e===void 0){let t=n!==void 0&&n.length===1;t&&(e=r.get(n)),e===void 0&&((this.o=e=new CSSStyleSheet).replaceSync(this.cssText),t&&r.set(n,e))}return e}toString(){return this.cssText}},a=e=>new i(typeof e==`string`?e:e+``,void 0,n),o=(e,...t)=>new i(e.length===1?e[0]:t.reduce((t,n,r)=>t+(e=>{if(!0===e._$cssResult$)return e.cssText;if(typeof e==`number`)return e;throw Error(`Value passed to 'css' function must be a 'css' function result: `+e+`. Use 'unsafeCSS' to pass non-literal values, but take care to ensure page security.`)})(n)+e[r+1],e[0]),e,n),s=(n,r)=>{if(t)n.adoptedStyleSheets=r.map(e=>e instanceof CSSStyleSheet?e:e.styleSheet);else for(let t of r){let r=document.createElement(`style`),i=e.litNonce;i!==void 0&&r.setAttribute(`nonce`,i),r.textContent=t.cssText,n.appendChild(r)}},c=t?e=>e:e=>e instanceof CSSStyleSheet?(e=>{let t=``;for(let n of e.cssRules)t+=n.cssText;return a(t)})(e):e,{is:l,defineProperty:u,getOwnPropertyDescriptor:ee,getOwnPropertyNames:te,getOwnPropertySymbols:ne,getPrototypeOf:re}=Object,ie=globalThis,ae=ie.trustedTypes,oe=ae?ae.emptyScript:``,se=ie.reactiveElementPolyfillSupport,ce=(e,t)=>e,d={toAttribute(e,t){switch(t){case Boolean:e=e?oe:null;break;case Object:case Array:e=e==null?e:JSON.stringify(e)}return e},fromAttribute(e,t){let n=e;switch(t){case Boolean:n=e!==null;break;case Number:n=e===null?null:Number(e);break;case Object:case Array:try{n=JSON.parse(e)}catch{n=null}}return n}},f=(e,t)=>!l(e,t),le={attribute:!0,type:String,converter:d,reflect:!1,useDefault:!1,hasChanged:f};Symbol.metadata??=Symbol(`metadata`),ie.litPropertyMetadata??=new WeakMap;var ue=class extends HTMLElement{static addInitializer(e){this._$Ei(),(this.l??=[]).push(e)}static get observedAttributes(){return this.finalize(),this._$Eh&&[...this._$Eh.keys()]}static createProperty(e,t=le){if(t.state&&(t.attribute=!1),this._$Ei(),this.prototype.hasOwnProperty(e)&&((t=Object.create(t)).wrapped=!0),this.elementProperties.set(e,t),!t.noAccessor){let n=Symbol(),r=this.getPropertyDescriptor(e,n,t);r!==void 0&&u(this.prototype,e,r)}}static getPropertyDescriptor(e,t,n){let{get:r,set:i}=ee(this.prototype,e)??{get(){return this[t]},set(e){this[t]=e}};return{get:r,set(t){let a=r?.call(this);i?.call(this,t),this.requestUpdate(e,a,n)},configurable:!0,enumerable:!0}}static getPropertyOptions(e){return this.elementProperties.get(e)??le}static _$Ei(){if(this.hasOwnProperty(ce(`elementProperties`)))return;let e=re(this);e.finalize(),e.l!==void 0&&(this.l=[...e.l]),this.elementProperties=new Map(e.elementProperties)}static finalize(){if(this.hasOwnProperty(ce(`finalized`)))return;if(this.finalized=!0,this._$Ei(),this.hasOwnProperty(ce(`properties`))){let e=this.properties,t=[...te(e),...ne(e)];for(let n of t)this.createProperty(n,e[n])}let e=this[Symbol.metadata];if(e!==null){let t=litPropertyMetadata.get(e);if(t!==void 0)for(let[e,n]of t)this.elementProperties.set(e,n)}this._$Eh=new Map;for(let[e,t]of this.elementProperties){let n=this._$Eu(e,t);n!==void 0&&this._$Eh.set(n,e)}this.elementStyles=this.finalizeStyles(this.styles)}static finalizeStyles(e){let t=[];if(Array.isArray(e)){let n=new Set(e.flat(1/0).reverse());for(let e of n)t.unshift(c(e))}else e!==void 0&&t.push(c(e));return t}static _$Eu(e,t){let n=t.attribute;return!1===n?void 0:typeof n==`string`?n:typeof e==`string`?e.toLowerCase():void 0}constructor(){super(),this._$Ep=void 0,this.isUpdatePending=!1,this.hasUpdated=!1,this._$Em=null,this._$Ev()}_$Ev(){this._$ES=new Promise(e=>this.enableUpdating=e),this._$AL=new Map,this._$E_(),this.requestUpdate(),this.constructor.l?.forEach(e=>e(this))}addController(e){(this._$EO??=new Set).add(e),this.renderRoot!==void 0&&this.isConnected&&e.hostConnected?.()}removeController(e){this._$EO?.delete(e)}_$E_(){let e=new Map,t=this.constructor.elementProperties;for(let n of t.keys())this.hasOwnProperty(n)&&(e.set(n,this[n]),delete this[n]);e.size>0&&(this._$Ep=e)}createRenderRoot(){let e=this.shadowRoot??this.attachShadow(this.constructor.shadowRootOptions);return s(e,this.constructor.elementStyles),e}connectedCallback(){this.renderRoot??=this.createRenderRoot(),this.enableUpdating(!0),this._$EO?.forEach(e=>e.hostConnected?.())}enableUpdating(e){}disconnectedCallback(){this._$EO?.forEach(e=>e.hostDisconnected?.())}attributeChangedCallback(e,t,n){this._$AK(e,n)}_$ET(e,t){let n=this.constructor.elementProperties.get(e),r=this.constructor._$Eu(e,n);if(r!==void 0&&!0===n.reflect){let i=(n.converter?.toAttribute===void 0?d:n.converter).toAttribute(t,n.type);this._$Em=e,i==null?this.removeAttribute(r):this.setAttribute(r,i),this._$Em=null}}_$AK(e,t){let n=this.constructor,r=n._$Eh.get(e);if(r!==void 0&&this._$Em!==r){let e=n.getPropertyOptions(r),i=typeof e.converter==`function`?{fromAttribute:e.converter}:e.converter?.fromAttribute===void 0?d:e.converter;this._$Em=r;let a=i.fromAttribute(t,e.type);this[r]=a??this._$Ej?.get(r)??a,this._$Em=null}}requestUpdate(e,t,n,r=!1,i){if(e!==void 0){let a=this.constructor;if(!1===r&&(i=this[e]),n??=a.getPropertyOptions(e),!((n.hasChanged??f)(i,t)||n.useDefault&&n.reflect&&i===this._$Ej?.get(e)&&!this.hasAttribute(a._$Eu(e,n))))return;this.C(e,t,n)}!1===this.isUpdatePending&&(this._$ES=this._$EP())}C(e,t,{useDefault:n,reflect:r,wrapped:i},a){n&&!(this._$Ej??=new Map).has(e)&&(this._$Ej.set(e,a??t??this[e]),!0!==i||a!==void 0)||(this._$AL.has(e)||(this.hasUpdated||n||(t=void 0),this._$AL.set(e,t)),!0===r&&this._$Em!==e&&(this._$Eq??=new Set).add(e))}async _$EP(){this.isUpdatePending=!0;try{await this._$ES}catch(e){Promise.reject(e)}let e=this.scheduleUpdate();return e!=null&&await e,!this.isUpdatePending}scheduleUpdate(){return this.performUpdate()}performUpdate(){if(!this.isUpdatePending)return;if(!this.hasUpdated){if(this.renderRoot??=this.createRenderRoot(),this._$Ep){for(let[e,t]of this._$Ep)this[e]=t;this._$Ep=void 0}let e=this.constructor.elementProperties;if(e.size>0)for(let[t,n]of e){let{wrapped:e}=n,r=this[t];!0!==e||this._$AL.has(t)||r===void 0||this.C(t,void 0,n,r)}}let e=!1,t=this._$AL;try{e=this.shouldUpdate(t),e?(this.willUpdate(t),this._$EO?.forEach(e=>e.hostUpdate?.()),this.update(t)):this._$EM()}catch(t){throw e=!1,this._$EM(),t}e&&this._$AE(t)}willUpdate(e){}_$AE(e){this._$EO?.forEach(e=>e.hostUpdated?.()),this.hasUpdated||(this.hasUpdated=!0,this.firstUpdated(e)),this.updated(e)}_$EM(){this._$AL=new Map,this.isUpdatePending=!1}get updateComplete(){return this.getUpdateComplete()}getUpdateComplete(){return this._$ES}shouldUpdate(e){return!0}update(e){this._$Eq&&=this._$Eq.forEach(e=>this._$ET(e,this[e])),this._$EM()}updated(e){}firstUpdated(e){}};ue.elementStyles=[],ue.shadowRootOptions={mode:`open`},ue[ce(`elementProperties`)]=new Map,ue[ce(`finalized`)]=new Map,se?.({ReactiveElement:ue}),(ie.reactiveElementVersions??=[]).push(`2.1.2`);var de=globalThis,p=e=>e,fe=de.trustedTypes,pe=fe?fe.createPolicy(`lit-html`,{createHTML:e=>e}):void 0,me=`$lit$`,m=`lit$${Math.random().toFixed(9).slice(2)}$`,he=`?`+m,ge=`<${he}>`,_e=document,ve=()=>_e.createComment(``),ye=e=>e===null||typeof e!=`object`&&typeof e!=`function`,be=Array.isArray,xe=e=>be(e)||typeof e?.[Symbol.iterator]==`function`,Se=`[ 	
\f\r]`,Ce=/<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g,we=/-->/g,Te=/>/g,Ee=RegExp(`>|${Se}(?:([^\\s"'>=/]+)(${Se}*=${Se}*(?:[^ \t\n\f\r"'\`<>=]|("|')|))|$)`,`g`),De=/'/g,Oe=/"/g,ke=/^(?:script|style|textarea|title)$/i,Ae=e=>(t,...n)=>({_$litType$:e,strings:t,values:n}),h=Ae(1),g=Ae(2),_=Symbol.for(`lit-noChange`),v=Symbol.for(`lit-nothing`),je=new WeakMap,Me=_e.createTreeWalker(_e,129);function Ne(e,t){if(!be(e)||!e.hasOwnProperty(`raw`))throw Error(`invalid template strings array`);return pe===void 0?t:pe.createHTML(t)}var Pe=(e,t)=>{let n=e.length-1,r=[],i,a=t===2?`<svg>`:t===3?`<math>`:``,o=Ce;for(let t=0;t<n;t++){let n=e[t],s,c,l=-1,u=0;for(;u<n.length&&(o.lastIndex=u,c=o.exec(n),c!==null);)u=o.lastIndex,o===Ce?c[1]===`!--`?o=we:c[1]===void 0?c[2]===void 0?c[3]!==void 0&&(o=Ee):(ke.test(c[2])&&(i=RegExp(`</`+c[2],`g`)),o=Ee):o=Te:o===Ee?c[0]===`>`?(o=i??Ce,l=-1):c[1]===void 0?l=-2:(l=o.lastIndex-c[2].length,s=c[1],o=c[3]===void 0?Ee:c[3]===`"`?Oe:De):o===Oe||o===De?o=Ee:o===we||o===Te?o=Ce:(o=Ee,i=void 0);let ee=o===Ee&&e[t+1].startsWith(`/>`)?` `:``;a+=o===Ce?n+ge:l>=0?(r.push(s),n.slice(0,l)+me+n.slice(l)+m+ee):n+m+(l===-2?t:ee)}return[Ne(e,a+(e[n]||`<?>`)+(t===2?`</svg>`:t===3?`</math>`:``)),r]},Fe=class e{constructor({strings:t,_$litType$:n},r){let i;this.parts=[];let a=0,o=0,s=t.length-1,c=this.parts,[l,u]=Pe(t,n);if(this.el=e.createElement(l,r),Me.currentNode=this.el.content,n===2||n===3){let e=this.el.content.firstChild;e.replaceWith(...e.childNodes)}for(;(i=Me.nextNode())!==null&&c.length<s;){if(i.nodeType===1){if(i.hasAttributes())for(let e of i.getAttributeNames())if(e.endsWith(me)){let t=u[o++],n=i.getAttribute(e).split(m),r=/([.?@])?(.*)/.exec(t);c.push({type:1,index:a,name:r[2],strings:n,ctor:r[1]===`.`?Be:r[1]===`?`?Ve:r[1]===`@`?He:ze}),i.removeAttribute(e)}else e.startsWith(m)&&(c.push({type:6,index:a}),i.removeAttribute(e));if(ke.test(i.tagName)){let e=i.textContent.split(m),t=e.length-1;if(t>0){i.textContent=fe?fe.emptyScript:``;for(let n=0;n<t;n++)i.append(e[n],ve()),Me.nextNode(),c.push({type:2,index:++a});i.append(e[t],ve())}}}else if(i.nodeType===8){if(i.data===he)c.push({type:2,index:a});else{let e=-1;for(;(e=i.data.indexOf(m,e+1))!==-1;)c.push({type:7,index:a}),e+=m.length-1}}a++}}static createElement(e,t){let n=_e.createElement(`template`);return n.innerHTML=e,n}};function Ie(e,t,n=e,r){if(t===_)return t;let i=r===void 0?n._$Cl:n._$Co?.[r],a=ye(t)?void 0:t._$litDirective$;return i?.constructor!==a&&(i?._$AO?.(!1),a===void 0?i=void 0:(i=new a(e),i._$AT(e,n,r)),r===void 0?n._$Cl=i:(n._$Co??=[])[r]=i),i!==void 0&&(t=Ie(e,i._$AS(e,t.values),i,r)),t}var Le=class{constructor(e,t){this._$AV=[],this._$AN=void 0,this._$AD=e,this._$AM=t}get parentNode(){return this._$AM.parentNode}get _$AU(){return this._$AM._$AU}u(e){let{el:{content:t},parts:n}=this._$AD,r=(e?.creationScope??_e).importNode(t,!0);Me.currentNode=r;let i=Me.nextNode(),a=0,o=0,s=n[0];for(;s!==void 0;){if(a===s.index){let t;s.type===2?t=new Re(i,i.nextSibling,this,e):s.type===1?t=new s.ctor(i,s.name,s.strings,this,e):s.type===6&&(t=new Ue(i,this,e)),this._$AV.push(t),s=n[++o]}a!==s?.index&&(i=Me.nextNode(),a++)}return Me.currentNode=_e,r}p(e){let t=0;for(let n of this._$AV)n!==void 0&&(n.strings===void 0?n._$AI(e[t]):(n._$AI(e,n,t),t+=n.strings.length-2)),t++}},Re=class e{get _$AU(){return this._$AM?._$AU??this._$Cv}constructor(e,t,n,r){this.type=2,this._$AH=v,this._$AN=void 0,this._$AA=e,this._$AB=t,this._$AM=n,this.options=r,this._$Cv=r?.isConnected??!0}get parentNode(){let e=this._$AA.parentNode,t=this._$AM;return t!==void 0&&e?.nodeType===11&&(e=t.parentNode),e}get startNode(){return this._$AA}get endNode(){return this._$AB}_$AI(e,t=this){e=Ie(this,e,t),ye(e)?e===v||e==null||e===``?(this._$AH!==v&&this._$AR(),this._$AH=v):e!==this._$AH&&e!==_&&this._(e):e._$litType$===void 0?e.nodeType===void 0?xe(e)?this.k(e):this._(e):this.T(e):this.$(e)}O(e){return this._$AA.parentNode.insertBefore(e,this._$AB)}T(e){this._$AH!==e&&(this._$AR(),this._$AH=this.O(e))}_(e){this._$AH!==v&&ye(this._$AH)?this._$AA.nextSibling.data=e:this.T(_e.createTextNode(e)),this._$AH=e}$(e){let{values:t,_$litType$:n}=e,r=typeof n==`number`?this._$AC(e):(n.el===void 0&&(n.el=Fe.createElement(Ne(n.h,n.h[0]),this.options)),n);if(this._$AH?._$AD===r)this._$AH.p(t);else{let e=new Le(r,this),n=e.u(this.options);e.p(t),this.T(n),this._$AH=e}}_$AC(e){let t=je.get(e.strings);return t===void 0&&je.set(e.strings,t=new Fe(e)),t}k(t){be(this._$AH)||(this._$AH=[],this._$AR());let n=this._$AH,r,i=0;for(let a of t)i===n.length?n.push(r=new e(this.O(ve()),this.O(ve()),this,this.options)):r=n[i],r._$AI(a),i++;i<n.length&&(this._$AR(r&&r._$AB.nextSibling,i),n.length=i)}_$AR(e=this._$AA.nextSibling,t){for(this._$AP?.(!1,!0,t);e!==this._$AB;){let t=p(e).nextSibling;p(e).remove(),e=t}}setConnected(e){this._$AM===void 0&&(this._$Cv=e,this._$AP?.(e))}},ze=class{get tagName(){return this.element.tagName}get _$AU(){return this._$AM._$AU}constructor(e,t,n,r,i){this.type=1,this._$AH=v,this._$AN=void 0,this.element=e,this.name=t,this._$AM=r,this.options=i,n.length>2||n[0]!==``||n[1]!==``?(this._$AH=Array(n.length-1).fill(new String),this.strings=n):this._$AH=v}_$AI(e,t=this,n,r){let i=this.strings,a=!1;if(i===void 0)e=Ie(this,e,t,0),a=!ye(e)||e!==this._$AH&&e!==_,a&&(this._$AH=e);else{let r=e,o,s;for(e=i[0],o=0;o<i.length-1;o++)s=Ie(this,r[n+o],t,o),s===_&&(s=this._$AH[o]),a||=!ye(s)||s!==this._$AH[o],s===v?e=v:e!==v&&(e+=(s??``)+i[o+1]),this._$AH[o]=s}a&&!r&&this.j(e)}j(e){e===v?this.element.removeAttribute(this.name):this.element.setAttribute(this.name,e??``)}},Be=class extends ze{constructor(){super(...arguments),this.type=3}j(e){this.element[this.name]=e===v?void 0:e}},Ve=class extends ze{constructor(){super(...arguments),this.type=4}j(e){this.element.toggleAttribute(this.name,!!e&&e!==v)}},He=class extends ze{constructor(e,t,n,r,i){super(e,t,n,r,i),this.type=5}_$AI(e,t=this){if((e=Ie(this,e,t,0)??v)===_)return;let n=this._$AH,r=e===v&&n!==v||e.capture!==n.capture||e.once!==n.once||e.passive!==n.passive,i=e!==v&&(n===v||r);r&&this.element.removeEventListener(this.name,this,n),i&&this.element.addEventListener(this.name,this,e),this._$AH=e}handleEvent(e){typeof this._$AH==`function`?this._$AH.call(this.options?.host??this.element,e):this._$AH.handleEvent(e)}},Ue=class{constructor(e,t,n){this.element=e,this.type=6,this._$AN=void 0,this._$AM=t,this.options=n}get _$AU(){return this._$AM._$AU}_$AI(e){Ie(this,e)}},We={M:me,P:m,A:he,C:1,L:Pe,R:Le,D:xe,V:Ie,I:Re,H:ze,N:Ve,U:He,B:Be,F:Ue},Ge=de.litHtmlPolyfillSupport;Ge?.(Fe,Re),(de.litHtmlVersions??=[]).push(`3.3.3`);var Ke=(e,t,n)=>{let r=n?.renderBefore??t,i=r._$litPart$;if(i===void 0){let e=n?.renderBefore??null;r._$litPart$=i=new Re(t.insertBefore(ve(),e),e,void 0,n??{})}return i._$AI(e),i},qe=globalThis,y=class extends ue{constructor(){super(...arguments),this.renderOptions={host:this},this._$Do=void 0}createRenderRoot(){let e=super.createRenderRoot();return this.renderOptions.renderBefore??=e.firstChild,e}update(e){let t=this.render();this.hasUpdated||(this.renderOptions.isConnected=this.isConnected),super.update(e),this._$Do=Ke(t,this.renderRoot,this.renderOptions)}connectedCallback(){super.connectedCallback(),this._$Do?.setConnected(!0)}disconnectedCallback(){super.disconnectedCallback(),this._$Do?.setConnected(!1)}render(){return _}};y._$litElement$=!0,y.finalized=!0,qe.litElementHydrateSupport?.({LitElement:y});var Je=qe.litElementPolyfillSupport;Je?.({LitElement:y}),(qe.litElementVersions??=[]).push(`4.2.2`);var b=e=>(t,n)=>{n===void 0?customElements.define(e,t):n.addInitializer(()=>{customElements.define(e,t)})},Ye={attribute:!0,type:String,converter:d,reflect:!1,hasChanged:f},Xe=(e=Ye,t,n)=>{let{kind:r,metadata:i}=n,a=globalThis.litPropertyMetadata.get(i);if(a===void 0&&globalThis.litPropertyMetadata.set(i,a=new Map),r===`setter`&&((e=Object.create(e)).wrapped=!0),a.set(n.name,e),r===`accessor`){let{name:r}=n;return{set(n){let i=t.get.call(this);t.set.call(this,n),this.requestUpdate(r,i,e,!0,n)},init(t){return t!==void 0&&this.C(r,void 0,e,t),t}}}if(r===`setter`){let{name:r}=n;return function(n){let i=this[r];t.call(this,n),this.requestUpdate(r,i,e,!0,n)}}throw Error(`Unsupported decorator location: `+r)};function x(e){return(t,n)=>typeof n==`object`?Xe(e,t,n):((e,t,n)=>{let r=t.hasOwnProperty(n);return t.constructor.createProperty(n,e),r?Object.getOwnPropertyDescriptor(t,n):void 0})(e,t,n)}function S(e){return x({...e,state:!0,attribute:!1})}var Ze=(e,t,n)=>(n.configurable=!0,n.enumerable=!0,Reflect.decorate&&typeof t!=`object`&&Object.defineProperty(e,t,n),n);function C(e,t){return(n,r,i)=>{let a=t=>t.renderRoot?.querySelector(e)??null;if(t){let{get:e,set:t}=typeof r==`object`?n:i??(()=>{let e=Symbol();return{get(){return this[e]},set(t){this[e]=t}}})();return Ze(n,r,{get(){let n=e.call(this);return n===void 0&&(n=a(this),(n!==null||this.hasUpdated)&&t.call(this,n)),n}})}return Ze(n,r,{get(){return a(this)}})}}var Qe=new Map,$e=new Set,et=`en`,tt;function nt(){for(let e of $e)e.refresh()}var rt=new Map,it=new Map;function at(e,t,n){let r=e.get(t);if(!r){try{r=n(t)}catch{r=n(et)}e.set(t,r)}return r}function ot(e,t){let n=e.toLowerCase(),r=Qe.get(n)?.[t];if(r!==void 0)return r;let i=n.split(`-`)[0];return i===n?void 0:Qe.get(i)?.[t]}function st(e,t,n,r={}){let i=ot(e,t)??n,a;if(typeof i==`string`)a=i;else{let t=Number(r.count??0);a=i[at(rt,e,e=>new Intl.PluralRules(e)).select(t)]??i.other}return a.includes(`{`)?a.replace(/\{(\w+)\}/g,(t,n)=>{let i=r[n];return i===void 0?t:typeof i==`number`?at(it,e,e=>new Intl.NumberFormat(e)).format(i):i}):a}function ct(e){let t=e;for(;t;){let e=t.closest(`[lang]`);if(e){let n=e.getAttribute(`lang`);if(n)return n;t=e.parentElement;continue}let n=t.getRootNode();t=n instanceof ShadowRoot?n.host:null}return et}function lt(e){try{return e.matches(`:dir(rtl)`)}catch{return getComputedStyle(e).direction===`rtl`}}var ut=class{constructor(e){this.host=e,e.addController(this)}hostConnected(){this.cachedLang=void 0,$e.add(this),!tt&&typeof MutationObserver<`u`&&(tt=new MutationObserver(()=>nt()),tt.observe(document.documentElement,{attributes:!0,attributeFilter:[`lang`,`dir`]}))}hostDisconnected(){$e.delete(this)}hostUpdated(){this.cachedLang=void 0}refresh(){this.cachedLang=void 0,this.host.requestUpdate()}get lang(){return this.cachedLang??=ct(this.host)}get dir(){return lt(this.host)?`rtl`:`ltr`}term(e,t,n){return st(this.lang,e,t,n)}number(e,t){try{return new Intl.NumberFormat(this.lang,t).format(e)}catch{return new Intl.NumberFormat(et,t).format(e)}}date(e,t){try{return new Intl.DateTimeFormat(this.lang,t).format(e)}catch{return new Intl.DateTimeFormat(et,t).format(e)}}},dt=o`
  :host {
    display: block;
    min-width: 0;
    /* An unbreakable token (an id, a URL) wraps instead of widening the page. */
    overflow-wrap: anywhere;
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
`,ft=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},pt={info:g`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <line x1="12" y1="11" x2="12" y2="16" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="7.5" r="1.25" fill="currentColor"></circle>
    </svg>
  `,success:g`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <polyline points="7,12.5 10.5,16 17,8.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></polyline>
    </svg>
  `,warning:g`
    <svg viewBox="0 0 24 24">
      <polygon points="12,3 22,20 2,20" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"></polygon>
      <line x1="12" y1="9" x2="12" y2="14" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="17" r="1.15" fill="currentColor"></circle>
    </svg>
  `,error:g`
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"></circle>
      <line x1="12" y1="7" x2="12" y2="13" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
      <circle cx="12" cy="16.5" r="1.25" fill="currentColor"></circle>
    </svg>
  `},mt=class extends y{constructor(){super(...arguments),this.variant=`info`,this.dismissible=!1,this.localize=new ut(this),this.roleIsOwn=!1}static{this.styles=dt}willUpdate(){(!this.hasAttribute(`role`)||this.roleIsOwn)&&(this.setAttribute(`role`,this.variant===`warning`||this.variant===`error`?`alert`:`status`),this.roleIsOwn=!0)}dismiss(){this.hidden=!0,this.dispatchEvent(new Event(`close`,{bubbles:!0}))}render(){return h`
      <div class="alert" part="alert">
        <span class="icon" part="icon" aria-hidden="true">
          <slot name="icon">${pt[this.variant]}</slot>
        </span>
        <div class="body" part="body">
          <slot name="title" class="title"></slot>
          <div class="description" part="description"><slot></slot></div>
          <slot name="action" class="actions"></slot>
        </div>
        ${this.dismissible?h`
              <button class="close" part="close" type="button" aria-label=${this.localize.term(`alert.dismiss`,`Dismiss`)} @click=${this.dismiss}>
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
                  <line x1="6" y1="18" x2="18" y2="6" stroke="currentColor" stroke-width="2" stroke-linecap="round"></line>
                </svg>
              </button>
            `:v}
      </div>
    `}};ft([x()],mt.prototype,`variant`,void 0),ft([x({type:Boolean,reflect:!0})],mt.prototype,`dismissible`,void 0),mt=ft([b(`lit-material-alert`)],mt);var ht=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},gt=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}};o`
  .control-text {
    margin-inline-start: 4px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-height: 40px;
    padding-block: 10px;
    box-sizing: border-box;
  }
  .control-text label {
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-body-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-large-size, 1rem);
    line-height: var(--md-sys-typescale-body-large-line-height, 1.5rem);
    cursor: pointer;
  }
  .control-text .supporting {
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
  }
  .control-text.disabled {
    opacity: 0.38;
  }
  .control-text.disabled label {
    cursor: default;
  }
`;var _t=o`
  :host {
    display: inline-flex;
    /* min-width 0 too: as a grid or flex item it would otherwise grow to its label's full width. */
    min-width: 0;
    max-width: 100%;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
  }

  :host([hidden]) {
    display: none;
  }

  :host([disabled]) {
    pointer-events: none;
  }

  .button {
    position: relative;
    display: inline-flex;
    /* Fill the host when a layout stretches it, so all of it is the button. */
    flex: 1 1 auto;
    align-items: center;
    justify-content: center;
    gap: 8px;
    box-sizing: border-box;
    height: var(--lit-material-button-height, var(--md-sys-density-button-height, 40px));
    min-width: min(64px, 100%);
    max-width: 100%;
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
    outline: var(--lit-material-focus-ring-width, 3px) solid var(--lit-material-focus-ring-color, var(--md-sys-color-secondary, #625b71));
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

  /* A label longer than the space it gets is cut with an ellipsis, not spilled out of the pill. */
  .label {
    position: relative;
    display: block;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) var(--lit-material-disabled-text-mix, 50%), transparent);
    box-shadow: none;
  }

  :host([variant="outlined"][disabled]) .button,
  :host([variant="text"][disabled]) .button {
    background-color: transparent;
  }

  /* The .focus-ring element is the one focus indicator: no second, browser-drawn ring on the inner control. */
  :where(button, a, input, [tabindex]):focus-visible {
    outline: none;
  }

  /* Touch: at least a 48px (44px for calendar days) target, the visual size unchanged where possible. */
  @media (pointer: coarse) {
    .button::after { content: ""; position: absolute; inset: -4px 0; }
  }
`,w=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},vt=0,T=class extends y{static{this.styles=_t}static{this.shadowRootOptions={...y.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`button`,this.disabled=!1,this.name=``,this.value=``,this.href=``,this.target=``,this.ripple=new ht(this),this.focusRing=new gt(this),this.labelId=`lit-material-button-label-${++vt}`,this.internals=this.attachInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}render(){let e=h`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      <slot name="icon" class="icon"></slot>
      <span class="label" id=${this.labelId}><slot></slot></span>
    `;return this.href?h`
        <a
          class="button"
          part="button"
          href=${this.disabled?v:this.href}
          target=${this.target||v}
          rel=${this.target===`_blank`?`noopener noreferrer`:v}
          aria-disabled=${this.disabled?`true`:v}
          aria-labelledby=${this.labelId}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${e}
        </a>
      `:h`
      <button
        class="button"
        part="button"
        type="button"
        ?disabled=${this.disabled}
        name=${this.name||v}
        value=${this.value||v}
        aria-labelledby=${this.labelId}
        @click=${this.handleClick}
      >
        ${e}
      </button>
    `}handleClick(e){this.disabled?(e.preventDefault(),e.stopImmediatePropagation()):this.type===`submit`?this.internals.form?.requestSubmit():this.type===`reset`&&this.internals.form?.reset()}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};w([x()],T.prototype,`variant`,void 0),w([x()],T.prototype,`type`,void 0),w([x({type:Boolean,reflect:!0})],T.prototype,`disabled`,void 0),w([x()],T.prototype,`name`,void 0),w([x()],T.prototype,`value`,void 0),w([x({reflect:!0})],T.prototype,`form`,void 0),w([x()],T.prototype,`href`,void 0),w([x()],T.prototype,`target`,void 0),w([C(`.state-layer`)],T.prototype,`stateLayerElement`,void 0),T=w([b(`lit-material-button`)],T);var yt=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},bt,xt=class extends y{constructor(){super(...arguments),this.align=`start`,this.compact=!1,this.label=``}static{bt=this}static{this.styles=o`
    :host {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--lit-material-action-list-gap, 8px);
    }
    :host([hidden]) {
      display: none;
    }
    :host([grouped]) {
      gap: var(--lit-material-action-list-gap, 16px);
    }
    :host([compact]) {
      gap: var(--lit-material-action-list-gap, 4px);
    }
    /* Aligning needs the room to align in: the full width, also as an item
       of a column flexbox, or the rest of a row flexbox's line. */
    :host([align="end"]),
    :host([align="between"]) {
      flex-grow: 1;
      align-self: stretch;
    }
    :host([align="end"]) {
      justify-content: flex-end;
    }
    :host([align="between"]) {
      justify-content: space-between;
    }
  `}updated(e){e.has(`label`)&&(this.label?(this.setAttribute(`role`,`group`),this.setAttribute(`aria-label`,this.label)):(this.removeAttribute(`role`),this.removeAttribute(`aria-label`)))}onSlotChange(){this.toggleAttribute(`grouped`,Array.from(this.children).some(e=>e instanceof bt))}render(){return h`<slot @slotchange=${this.onSlotChange}></slot>`}};yt([x({reflect:!0})],xt.prototype,`align`,void 0),yt([x({type:Boolean,reflect:!0})],xt.prototype,`compact`,void 0),yt([x()],xt.prototype,`label`,void 0),xt=bt=yt([b(`lit-material-action-list`)],xt);var St=o`
  :host {
    display: inline-flex;
    vertical-align: middle;
  }

  :host([hidden]) {
    display: none;
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

  :host([mono]) .status {
    font-family: var(--lit-material-status-mono-font, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-variant-numeric: tabular-nums;
  }

  :host([appearance="pill"]) .status {
    padding: 2px 8px;
    border-radius: var(--md-sys-shape-corner-full, 9999px);
  }
  :host([appearance="pill"]) .icon { display: none; }
  :host([appearance="pill"]) .ok {
    background: var(--lit-material-status-ok-container, light-dark(#dcebd3, #23361a));
    color: var(--lit-material-status-on-ok-container, light-dark(#2a4a1b, #b7e2a1));
  }
  :host([appearance="pill"]) .warning {
    background: var(--lit-material-status-warning-container, light-dark(#ffe1a8, #3d2c00));
    color: var(--lit-material-status-on-warning-container, light-dark(#4d3300, #ffd98a));
  }
  :host([appearance="pill"]) .error {
    background: var(--md-sys-color-error-container, light-dark(#f9dedc, #8c1d18));
    color: var(--md-sys-color-on-error-container, light-dark(#410e0b, #f9dedc));
  }
  :host([appearance="pill"]) .info {
    background: var(--md-sys-color-primary-container, light-dark(#eaddff, #4f378b));
    color: var(--md-sys-color-on-primary-container, light-dark(#21005d, #eaddff));
  }
  :host([appearance="pill"]) .neutral {
    background: var(--md-sys-color-surface-container-highest, light-dark(#e6e0e9, #36343b));
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
  }

  @media (forced-colors: active) {
    .status { color: CanvasText; }
    :host([appearance="pill"]) .status { border: 1px solid CanvasText; background: Canvas; }
  }
`,Ct=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},wt={ok:g`<circle cx="10" cy="10" r="8"/><path d="M6.5 10.2l2.4 2.4 4.6-4.8"/>`,warning:g`<path d="M10 3l7.5 13h-15z"/><path d="M10 8.2v3.6M10 14.2v.1"/>`,error:g`<circle cx="10" cy="10" r="8"/><path d="M7.2 7.2l5.6 5.6M12.8 7.2l-5.6 5.6"/>`,info:g`<circle cx="10" cy="10" r="8"/><path d="M10 9v5M10 6.2v.1"/>`,neutral:g`<circle cx="10" cy="10" r="3.2" class="fill"/>`},Tt=new Set([`ok`,`warning`,`error`,`info`,`neutral`]),Et=class extends y{constructor(){super(...arguments),this.status=`neutral`,this.appearance=`text`,this.mono=!1}static{this.styles=St}get kind(){return Tt.has(this.status)?this.status:`neutral`}render(){let e=this.kind;return h`<span class="status ${e}" part="status">
      <span class="icon" part="icon" aria-hidden="true"
        ><slot name="icon"><svg viewBox="0 0 20 20" focusable="false">${wt[e]}</svg></slot></span
      ><span class="label"><slot></slot></span>
    </span>`}};Ct([x({reflect:!0})],Et.prototype,`status`,void 0),Ct([x({reflect:!0})],Et.prototype,`appearance`,void 0),Ct([x({type:Boolean,reflect:!0})],Et.prototype,`mono`,void 0),Et=Ct([b(`lit-material-status`)],Et);var Dt=o`
  :host {
    display: block;
  }

  :host([hidden]) {
    display: none;
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
`,Ot=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},kt=class extends y{constructor(){super(...arguments),this.headline=``,this.level=2,this.size=`standard`,this.emptyIcon=!1,this.emptyActions=!1}static{this.styles=Dt}onSlotChange(e){let t=e.target,n=t.assignedNodes({flatten:!0}).length===0;t.name===`icon`?this.emptyIcon=n:this.emptyActions=n}firstUpdated(){for(let e of this.renderRoot.querySelectorAll(`slot[name]`))e.dispatchEvent(new Event(`slotchange`))}render(){let e=Math.min(6,Math.max(1,Math.round(this.level||2)));return h`
      <div class="empty-state ${this.size}" part="empty-state">
        <div class="icon ${this.emptyIcon?`empty`:``}"><slot name="icon" @slotchange=${this.onSlotChange}></slot></div>
        ${this.headline?h`<div class="headline" part="headline" role="heading" aria-level=${e}>${this.headline}</div>`:v}
        <div class="body"><slot></slot></div>
        <div class="actions ${this.emptyActions?`empty`:``}"><slot name="actions" @slotchange=${this.onSlotChange}></slot></div>
      </div>
    `}};Ot([x()],kt.prototype,`headline`,void 0),Ot([x({type:Number})],kt.prototype,`level`,void 0),Ot([x({reflect:!0})],kt.prototype,`size`,void 0),Ot([S()],kt.prototype,`emptyIcon`,void 0),Ot([S()],kt.prototype,`emptyActions`,void 0),kt=Ot([b(`lit-material-empty-state`)],kt);var At=new Map,jt=new Set,Mt=`en`,Nt;function Pt(){for(let e of jt)e.refresh()}var Ft=new Map,It=new Map;function Lt(e,t,n){let r=e.get(t);if(!r){try{r=n(t)}catch{r=n(Mt)}e.set(t,r)}return r}function Rt(e,t){let n=e.toLowerCase(),r=At.get(n)?.[t];if(r!==void 0)return r;let i=n.split(`-`)[0];return i===n?void 0:At.get(i)?.[t]}function zt(e,t,n,r={}){let i=Rt(e,t)??n,a;if(typeof i==`string`)a=i;else{let t=Number(r.count??0);a=i[Lt(Ft,e,e=>new Intl.PluralRules(e)).select(t)]??i.other}return a.includes(`{`)?a.replace(/\{(\w+)\}/g,(t,n)=>{let i=r[n];return i===void 0?t:typeof i==`number`?Lt(It,e,e=>new Intl.NumberFormat(e)).format(i):i}):a}function Bt(e){let t=e;for(;t;){let e=t.closest(`[lang]`);if(e){let n=e.getAttribute(`lang`);if(n)return n;t=e.parentElement;continue}let n=t.getRootNode();t=n instanceof ShadowRoot?n.host:null}return Mt}function Vt(e){try{return e.matches(`:dir(rtl)`)}catch{return getComputedStyle(e).direction===`rtl`}}var Ht=class{constructor(e){this.host=e,e.addController(this)}hostConnected(){this.cachedLang=void 0,jt.add(this),!Nt&&typeof MutationObserver<`u`&&(Nt=new MutationObserver(()=>Pt()),Nt.observe(document.documentElement,{attributes:!0,attributeFilter:[`lang`,`dir`]}))}hostDisconnected(){jt.delete(this)}hostUpdated(){this.cachedLang=void 0}refresh(){this.cachedLang=void 0,this.host.requestUpdate()}get lang(){return this.cachedLang??=Bt(this.host)}get dir(){return Vt(this.host)?`rtl`:`ltr`}term(e,t,n){return zt(this.lang,e,t,n)}number(e,t){try{return new Intl.NumberFormat(this.lang,t).format(e)}catch{return new Intl.NumberFormat(Mt,t).format(e)}}date(e,t){try{return new Intl.DateTimeFormat(this.lang,t).format(e)}catch{return new Intl.DateTimeFormat(Mt,t).format(e)}}};function Ut(e,t){if(!(e>0)||!Number.isFinite(e))return 1;let n=e/Math.max(1,t),r=10**Math.floor(Math.log10(n));for(let e of[1,2,2.5,5,10])if(e*r>=n)return e*r;return 10*r}function Wt(e,t,n=4){if(!Number.isFinite(e)||!Number.isFinite(t))return{ticks:[0,1],min:0,max:1};if(e===t){let n=e===0?1:Math.abs(e)*.1;e-=n,t+=n}let r=Ut(t-e,n),i=Math.floor(e/r)*r,a=Math.ceil(t/r)*r,o=[];for(let e=i,t=0;e<=a+r/2&&t<n*3+2;e+=r,t++)o.push(Number(e.toPrecision(12)));return{ticks:o,min:i,max:a}}function Gt(e,t,n=5){return!Number.isFinite(e)||!Number.isFinite(t)?[]:e===t?[e]:Array.from({length:n},(r,i)=>e+(t-e)*i/(n-1))}function Kt(e,t){if(!Number.isFinite(e))return``;let n=Math.abs(e),r=e=>t?Yt(t).format(e):String(e);return n>=1e9?`${r(qt(e/1e9))}G`:n>=1e6?`${r(qt(e/1e6))}M`:n>=1e4?`${r(qt(e/1e3))}k`:r(n>=100||Number.isInteger(e)?Math.round(e):qt(e))}function qt(e){return Number(e.toPrecision(3))}var Jt=new Map;function Yt(e){let t=Jt.get(e);if(!t){let n={useGrouping:!1,maximumFractionDigits:20};try{t=new Intl.NumberFormat(e,n)}catch{t=new Intl.NumberFormat(`en`,n)}Jt.set(e,t)}return t}function Xt(e){let t=e.indexOf(``);return t<0?[e,``]:[e.slice(0,t),e.slice(t+1)]}function Zt(e,t,n,r){let i=e.map((e,t)=>({p:e,i:t})).sort((e,t)=>e.p-t.p),a=[...e],o=-1/0;for(let{p:e,i:r}of i){let i=Math.max(e,o+t,n);a[r]=i,o=i}let s=Math.max(0,Math.max(...a)-r);return s?a.map(e=>e-s):a}var Qt=a([[`#2a78d6`,`#3987e5`],[`#eb6834`,`#d95926`],[`#1baf7a`,`#199e70`],[`#eda100`,`#c98500`],[`#e87ba4`,`#d55181`],[`#008300`,`#008300`],[`#6250d6`,`#9085e9`],[`#e34948`,`#e66767`]].map(([e,t],n)=>`.s${n+1} { --series: var(--lit-material-chart-series-${n+1}, light-dark(${e}, ${t})); }`).join(`
`)),$t=o`
  :host {
    --chart-text: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    --chart-muted: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    --chart-grid: var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    --chart-surface: var(--md-sys-color-surface, light-dark(#fef7ff, #141218));
  }
`,en=[$t,Qt,o`
    :host {
      display: block;
      min-width: 0;
      color: var(--chart-text);
      font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    }

    :host([hidden]) {
      display: none;
    }
    .chart {
      position: relative;
    }
    /* The plot is geometry: its x axis runs left to right in right-to-left pages too. LTR here also keeps
       SVG text-anchor start/end meaning left/right (series names still read right to left within). */
    svg {
      direction: ltr;
      display: block;
      width: 100%;
      height: auto;
      overflow: visible;
      touch-action: pan-y;
    }
    svg:focus-visible {
      outline: var(--lit-material-focus-ring-width, 3px) solid var(--lit-material-focus-ring-color, var(--md-sys-color-secondary, #625b71));
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
  `],E={ATTRIBUTE:1,CHILD:2,PROPERTY:3,BOOLEAN_ATTRIBUTE:4,EVENT:5,ELEMENT:6},tn=e=>(...t)=>({_$litDirective$:e,values:t}),nn=class{constructor(e){}get _$AU(){return this._$AM._$AU}_$AT(e,t,n){this._$Ct=e,this._$AM=t,this._$Ci=n}_$AS(e,t){return this.update(e,t)}update(e,t){return this.render(...t)}},rn=e=>e.startsWith(`--`)?e:e.replace(/[A-Z]/g,e=>`-${e.toLowerCase()}`),an=e=>e!=null&&e!==``,on=tn(class extends nn{constructor(e){if(super(e),this.previous=new Set,e.type!==E.ATTRIBUTE||e.name!==`style`||(e.strings?.length??0)>2)throw Error("cssProps() must be the whole value of a style attribute: style=${cssProps({...})}")}render(e){return Object.entries(e).filter(([,e])=>an(e)).map(([e,t])=>`${rn(e)}: ${t}`).join(`; `)}update(e,[t]){let{style:n}=e.element,r=new Set;for(let[e,i]of Object.entries(t)){if(!an(i))continue;let t=rn(e);r.add(t),n.setProperty(t,String(i))}for(let e of this.previous)r.has(e)||n.removeProperty(e);return this.previous=r,_}}),D=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},sn=4,cn=8,ln=28,un=40,O=class extends y{constructor(){super(...arguments),this.series=[],this.label=``,this.height=240,this.unit=``,this.width=640,this.active=-1,this.localize=new Ht(this)}static{this.styles=en}fx(e){return this.formatX?this.formatX(e):Kt(e,this.localize.lang)}fy(e){return this.formatY?this.formatY(e):Kt(e,this.localize.lang)}withUnit(e){return this.unit?this.localize.term(`chart.valueWithUnit`,`{value} {unit}`,{value:e,unit:this.unit}):e}connectedCallback(){super.connectedCallback(),typeof ResizeObserver<`u`&&(this.observer=new ResizeObserver(e=>{let t=Math.round(e[0]?.contentRect.width??0);t>0&&t!==this.width&&requestAnimationFrame(()=>this.width=t)}),this.observer.observe(this))}disconnectedCallback(){super.disconnectedCallback(),this.observer?.disconnect()}get drawn(){return this.series.length>8&&console.warn(`lit-material-line-chart: ${this.series.length} series; only the first 8 are drawn`),this.series.slice(0,8).map(e=>({name:e.name,points:e.points.filter(e=>Number.isFinite(e.x)&&Number.isFinite(e.y)).sort((e,t)=>e.x-t.x)}))}xsOf(e){let t=new Set;for(let n of e)for(let e of n.points)t.add(e.x);return[...t].sort((e,t)=>e-t)}nearest(e,t){let n;for(let r of e)(!n||Math.abs(r.x-t)<Math.abs(n.x-t))&&(n=r);return n}onPointer(e,t,n){let r=e.currentTarget.getBoundingClientRect(),i=(e.clientX-r.left)/Math.max(1,r.width)*this.width,a=0;for(let e=1;e<t.length;e++)Math.abs(n(t[e])-i)<Math.abs(n(t[a])-i)&&(a=e);this.active=a}onKey(e,t){let n={ArrowLeft:-1,ArrowRight:1,Home:-1/0,End:1/0};if(e.key===`Escape`){this.active=-1;return}if(!(e.key in n))return;e.preventDefault();let r=n[e.key],i=this.active<0?t-1:this.active;this.active=Math.min(t-1,Math.max(0,Number.isFinite(r)?i+r:r<0?0:t-1))}summary(e){return e.map(e=>{let t=e.points.map(e=>e.y);if(!t.length)return this.localize.term(`chart.nameNoData`,`{name}: no data`,{name:e.name});let n=t[t.length-1];return this.localize.term(`chart.seriesSummary`,`{name}: latest {latest}, range {min} to {max}`,{name:e.name,latest:this.withUnit(this.fy(n)),min:this.fy(Math.min(...t)),max:this.fy(Math.max(...t))})}).join(this.localize.term(`chart.listSeparator`,`; `))}render(){let e=this.drawn,t=this.xsOf(e),n=this.label||this.localize.term(`chart.lineChart`,`Line chart`);if(t.length===0)return h`<div class="empty" role="img" aria-label=${this.localize.term(`chart.nameNoData`,`{name}: no data`,{name:n})}>${this.localize.term(`chart.noData`,`No data`)}</div>`;let r=e.flatMap(e=>e.points.map(e=>e.y)),i=Math.min(...r),a=this.yMin??Math.min(0,i),{ticks:o,min:s,max:c}=Wt(Math.min(a,i),Math.max(...r),4),l=e.length>1&&e.length<=sn,u=Math.max(0,...e.map(e=>e.name.length)),ee=l?Math.min(160,12+u*7):12,te=o.map(e=>this.fy(e)),ne=Math.max(un,12+Math.max(...te.map(e=>e.length))*7),re=this.width,ie=Math.max(80,this.height),ae=Math.max(1,re-ne-ee),oe=Math.max(1,ie-cn-ln),se=t[0],ce=t[t.length-1],d=e=>ne+(ce===se?ae/2:(e-se)/(ce-se)*ae),f=e=>cn+(1-(e-s)/(c-s||1))*oe,le=Gt(se,ce,Math.max(2,Math.min(6,Math.floor(ae/110)+1))),ue=e.map(e=>e.points.length?f(e.points[e.points.length-1].y):cn),de=l?Zt(ue,14,14,cn+oe):[],p=this.active>=0&&this.active<t.length?t[this.active]:void 0,fe=p===void 0?[]:e.map((e,t)=>({i:t,name:e.name,point:this.nearest(e.points,p)})).filter(e=>e.point),pe=p===void 0?0:d(p)/re*100;return h`
      ${e.length>1?h`<ul class="legend" part="legend" aria-hidden="true">
            ${e.map((e,t)=>h`<li><span class="swatch s${t+1}"></span>${e.name}</li>`)}
          </ul>`:v}
      <div class="chart" part="chart">
        <svg
          viewBox="0 0 ${re} ${ie}"
          width=${re}
          height=${ie}
          role="img"
          aria-label=${this.localize.term(`chart.labelSummary`,`{label}. {summary}`,{label:n,summary:this.summary(e)})}
          tabindex="0"
          @pointermove=${e=>this.onPointer(e,t,d)}
          @pointerdown=${e=>this.onPointer(e,t,d)}
          @pointerleave=${()=>this.active=-1}
          @keydown=${e=>this.onKey(e,t.length)}
          @blur=${()=>this.active=-1}
        >
          ${o.map((e,t)=>g`
              <line class="grid" x1=${ne} x2=${ne+ae} y1=${f(e)} y2=${f(e)}></line>
              <text class="axis" x=${ne-8} y=${f(e)+4} text-anchor="end">${te[t]}</text>`)}
          ${le.map((e,t)=>g`<text class="axis" x=${d(e)} y=${ie-8}
              text-anchor=${t===0?`start`:t===le.length-1?`end`:`middle`}>${this.fx(e)}</text>`)}
          ${e.map((e,t)=>e.points.length===1?g`<circle class="dot s${t+1}" r="4" cx=${d(e.points[0].x)} cy=${f(e.points[0].y)}></circle>`:g`<polyline class="line s${t+1}" points=${e.points.map(e=>`${d(e.x).toFixed(1)},${f(e.y).toFixed(1)}`).join(` `)}></polyline>`)}
          ${l?e.map((e,t)=>g`<text class="end-label" x=${ne+ae+6} y=${(de[t]??cn)+4}>${e.name}</text>`):v}
          ${p===void 0?v:g`<line class="crosshair" x1=${d(p)} x2=${d(p)} y1=${cn} y2=${cn+oe}></line>
                ${fe.map(e=>g`<circle class="marker s${e.i+1}" r="4" cx=${d(e.point.x)} cy=${f(e.point.y)}></circle>`)}`}
        </svg>
        ${p===void 0?v:h`<div class="tooltip" part="tooltip" role="status" style=${on({left:`${pe}%`})}>
              <div class="tip-x">${this.fx(p)}</div>
              ${fe.map(t=>{let n=this.withUnit(``),[r,i]=Xt(e.length>1?this.localize.term(`chart.nameValue`,`{name}: {value}`,{name:t.name,value:n}):n);return h`<div class="tip-row">${e.length>1?h`<span class="swatch s${t.i+1}"></span>`:v}<span
                    >${r}<strong>${this.fy(t.point.y)}</strong>${i}</span
                  ></div>`})}
            </div>`}
      </div>
      <table class="visually-hidden">
        <caption>${n}</caption>
        <thead>
          <tr>
            <th scope="col">x</th>
            ${e.map(e=>h`<th scope="col">${this.unit?this.localize.term(`chart.nameInUnit`,`{name} ({unit})`,{name:e.name,unit:this.unit}):e.name}</th>`)}
          </tr>
        </thead>
        <tbody>
          ${t.slice(-200).map(t=>h`<tr>
                <th scope="row">${this.fx(t)}</th>
                ${e.map(e=>{let n=e.points.find(e=>e.x===t);return h`<td>${n?this.fy(n.y):``}</td>`})}
              </tr>`)}
        </tbody>
      </table>
    `}};D([x({type:Array})],O.prototype,`series`,void 0),D([x()],O.prototype,`label`,void 0),D([x({type:Number})],O.prototype,`height`,void 0),D([x({type:Number,attribute:`y-min`})],O.prototype,`yMin`,void 0),D([x({attribute:!1})],O.prototype,`formatX`,void 0),D([x({attribute:!1})],O.prototype,`formatY`,void 0),D([x()],O.prototype,`unit`,void 0),D([S()],O.prototype,`width`,void 0),D([S()],O.prototype,`active`,void 0),O=D([b(`lit-material-line-chart`)],O);var dn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},fn=class extends y{constructor(){super(...arguments),this.values=[],this.label=``,this.showLast=!1,this.localize=new Ht(this)}static{this.styles=[$t,Qt,o`
      :host {
        display: inline-block;
        width: var(--lit-material-sparkline-width, 96px);
        height: var(--lit-material-sparkline-height, 24px);
        vertical-align: middle;
      }

      :host([hidden]) {
        display: none;
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
    `]}formatValue(e){return this.format?this.format(e):Kt(e,this.localize.lang)}render(){let e=this.values.filter(e=>Number.isFinite(e)),t=this.label||this.localize.term(`chart.trend`,`Trend`);if(e.length===0)return h`<svg role="img" aria-label=${this.localize.term(`chart.nameNoData`,`{name}: no data`,{name:t})}></svg>`;let n=Math.min(...e),r=Math.max(...e),i=r-n||1,a=t=>e.length===1?50:t/(e.length-1)*100,o=e=>2+(1-(e-n)/i)*20,s=e[e.length-1];return h`<svg class="s1" viewBox="0 0 ${100} ${24}" preserveAspectRatio="none" role="img"
      aria-label=${this.localize.term(`chart.sparklineSummary`,`{name}: latest {latest}, from {min} to {max}`,{name:t,latest:this.formatValue(s),min:this.formatValue(n),max:this.formatValue(r)})}>
      <polyline points=${e.map((e,t)=>`${a(t).toFixed(2)},${o(e).toFixed(2)}`).join(` `)}></polyline>
      ${this.showLast||e.length===1?g`<circle cx=${a(e.length-1)} cy=${o(s)} r="2.5"></circle>`:v}
    </svg>`}};dn([x({type:Array})],fn.prototype,`values`,void 0),dn([x()],fn.prototype,`label`,void 0),dn([x({type:Boolean,attribute:`show-last`})],fn.prototype,`showLast`,void 0),dn([x({attribute:!1})],fn.prototype,`format`,void 0),fn=dn([b(`lit-material-sparkline`)],fn);var pn=[$t,o`
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

    :host([hidden]) {
      display: none;
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
      outline: var(--lit-material-focus-ring-width, 3px) solid var(--lit-material-focus-ring-color, var(--md-sys-color-secondary, #625b71));
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
      inset-inline-start: 0;
      top: 2px;
      height: 16px;
      width: max(2px, calc((100% - var(--label)) * var(--f)));
      border-start-end-radius: 4px;
      border-end-end-radius: 4px;
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
      inset-inline-start: calc((100% - var(--label)) * var(--f) + 6px);
      white-space: nowrap;
      font-variant-numeric: tabular-nums;
      font-weight: 600;
    }

    .tooltip {
      position: absolute;
      inset-inline-end: 0;
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
  `],mn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},k=class extends y{constructor(){super(...arguments),this.data=[],this.label=``,this.highlight=``,this.sort=``,this.unit=``,this.localize=new Ht(this),this.active=-1}static{this.styles=pn}get rows(){let e=this.data.filter(e=>Number.isFinite(e.value));return this.sort===`desc`?e.sort((e,t)=>t.value-e.value):this.sort===`asc`&&e.sort((e,t)=>e.value-t.value),e}formatValue(e){return this.format?this.format(e):Kt(e,this.localize.lang)}withUnit(e){return this.unit?this.localize.term(`chart.valueWithUnit`,`{value} {unit}`,{value:e,unit:this.unit}):e}text(e){return this.withUnit(this.formatValue(e.value))}onKey(e,t){if(e.key===`Escape`){this.active=-1;return}let n={ArrowDown:e=>e+1,ArrowUp:e=>e<0?t-1:e-1,Home:()=>0,End:()=>t-1}[e.key];n&&(e.preventDefault(),this.active=Math.min(t-1,Math.max(0,n(this.active))))}renderReadout(e){let[t,n]=Xt(this.localize.term(`chart.nameValue`,`{name}: {value}`,{name:e.name,value:``}));return h`${t}<strong>${this.text(e)}</strong>${n}`}render(){let e=this.rows,t=this.label||this.localize.term(`chart.barChart`,`Bar chart`);if(e.length===0)return h`<div class="empty" role="img" aria-label=${this.localize.term(`chart.nameNoData`,`{name}: no data`,{name:t})}>${this.localize.term(`chart.noData`,`No data`)}</div>`;let n=Math.max(0,...e.map(e=>e.value))||1,r=this.highlight!==``&&e.some(e=>e.name===this.highlight),i=e[this.active];return h`
      <div
        class="chart"
        part="chart"
        role="img"
        aria-label=${this.localize.term(`chart.labelSummary`,`{label}. {summary}`,{label:t,summary:e.map(e=>this.localize.term(`chart.nameValue`,`{name}: {value}`,{name:e.name,value:this.text(e)})).join(this.localize.term(`chart.listSeparator`,`; `))})}
        tabindex="0"
        @keydown=${t=>this.onKey(t,e.length)}
        @pointerleave=${()=>this.active=-1}
        @blur=${()=>this.active=-1}
      >
        ${e.map((e,t)=>{let i=r&&e.name===this.highlight,a=Math.max(0,e.value)/n;return h`<div class=${[`row`,i?`highlight`:``,r&&!i?`muted`:``,t===this.active?`active`:``].join(` `)} aria-hidden="true" @pointerenter=${()=>this.active=t} @pointerdown=${()=>this.active=t}>
            <span class="name">${e.name}</span>
            <span class="track" style=${on({"--f":a.toFixed(4)})}>
              <span class="bar"></span>
              ${!r||i?h`<span class="value">${this.text(e)}</span>`:v}
            </span>
          </div>`})}
        ${i?h`<div class="tooltip" part="tooltip" role="status" style=${on({top:`calc(${this.active} * var(--row-pitch))`})}>
              ${this.renderReadout(i)}
            </div>`:v}
      </div>
      <table class="visually-hidden">
        <caption>${t}</caption>
        <thead>
          <tr>
            <th scope="col">${this.localize.term(`chart.name`,`Name`)}</th>
            <th scope="col">${this.unit?this.localize.term(`chart.valueInUnit`,`Value ({unit})`,{unit:this.unit}):this.localize.term(`chart.value`,`Value`)}</th>
          </tr>
        </thead>
        <tbody>
          ${e.map(e=>h`<tr><th scope="row">${e.name}</th><td>${this.formatValue(e.value)}</td></tr>`)}
        </tbody>
      </table>
    `}};mn([x({type:Array})],k.prototype,`data`,void 0),mn([x()],k.prototype,`label`,void 0),mn([x()],k.prototype,`highlight`,void 0),mn([x()],k.prototype,`sort`,void 0),mn([x()],k.prototype,`unit`,void 0),mn([x({attribute:!1})],k.prototype,`format`,void 0),mn([S()],k.prototype,`active`,void 0),k=mn([b(`lit-material-bar-chart`)],k);var hn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},gn=class extends y{constructor(){super(...arguments),this.items=[],this.label=``,this.localize=new Ht(this)}static{this.styles=[$t,o`
      :host {
        --pass: var(--lit-material-status-strip-pass, light-dark(#2f7d4f, #3fa77f));
        --fail: var(--lit-material-status-strip-fail, var(--md-sys-color-error, light-dark(#b3261e, #f2b8b5)));
        --warn: var(--lit-material-status-strip-warn, light-dark(#8a5a12, #f2b45c));
        --none: var(--lit-material-status-strip-none, var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f)));
        display: block;
        min-width: 48px;
      }
      :host([hidden]) {
        display: none;
      }
      .strip {
        display: flex;
        align-items: stretch;
        gap: var(--lit-material-status-strip-gap, 2px);
        height: var(--lit-material-status-strip-height, 24px);
      }
      .bar {
        flex: 1;
        min-width: 2px;
        border-radius: 2px;
      }
      .pass { background: var(--pass); }
      .warn { background: var(--warn); }
      .none { background: var(--none); }
      .fail {
        background: repeating-linear-gradient(135deg, var(--fail) 0 3px, color-mix(in srgb, var(--fail) 70%, transparent) 3px 5px);
      }
      @media (forced-colors: active) {
        .bar { border: 1px solid CanvasText; background: Canvas; }
        .fail { background: CanvasText; }
        .warn { background: repeating-linear-gradient(135deg, CanvasText 0 2px, Canvas 2px 4px); }
      }
    `]}get summary(){let e=this.localize,t=this.label||e.term(`chart.runs`,`Runs`),n=(this.items??[]).filter(e=>e&&e.status!==`none`);if(n.length===0)return e.term(`chart.nameNoData`,`{name}: no data`,{name:t});let r=n.filter(e=>e.status===`pass`).length,i=n.filter(e=>e.status===`fail`).length,a=n[n.length-1].status,o=a===`pass`?e.term(`chart.runPassed`,`passed`):a===`fail`?e.term(`chart.runFailed`,`failed`):e.term(`chart.runWarned`,`warned`);return e.term(`chart.statusStripSummary`,`{name}: {passed} of {total} passed, {failed} failed, latest {latest}`,{name:t,passed:r,total:n.length,failed:i,latest:o})}render(){let e=this.items??[];return h`<div class="strip" part="strip" role="img" aria-label=${this.summary}>
      ${e.map(e=>h`<span class="bar ${[`pass`,`fail`,`warn`].includes(e?.status)?e.status:`none`}" part="bar" title=${e?.label??``}></span>`)}
    </div>`}};hn([x({type:Array})],gn.prototype,`items`,void 0),hn([x()],gn.prototype,`label`,void 0),gn=hn([b(`lit-material-status-strip`)],gn);var _n=o`
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

  /* Right to left, the wave sweeps in reading direction: from the right edge. */
  @keyframes lit-material-skeleton-wave-rtl {
    0% {
      transform: translateX(100%);
    }
    100% {
      transform: translateX(-100%);
    }
  }

  :host {
    display: block;
  }

  :host([hidden]) {
    display: none;
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

  :host([animation="wave"]:dir(rtl)) .skeleton::after {
    transform: translateX(100%);
    animation-name: lit-material-skeleton-wave-rtl;
  }

  /* !important: the :host(...) rules above are more specific than these. */
  @media (prefers-reduced-motion: reduce) {
    .skeleton,
    .skeleton::after {
      animation: none !important;
    }
  }
`,vn=e=>e.startsWith(`--`)?e:e.replace(/[A-Z]/g,e=>`-${e.toLowerCase()}`),yn=e=>e!=null&&e!==``,bn=tn(class extends nn{constructor(e){if(super(e),this.previous=new Set,e.type!==E.ATTRIBUTE||e.name!==`style`||(e.strings?.length??0)>2)throw Error("cssProps() must be the whole value of a style attribute: style=${cssProps({...})}")}render(e){return Object.entries(e).filter(([,e])=>yn(e)).map(([e,t])=>`${vn(e)}: ${t}`).join(`; `)}update(e,[t]){let{style:n}=e.element,r=new Set;for(let[e,i]of Object.entries(t)){if(!yn(i))continue;let t=vn(e);r.add(t),n.setProperty(t,String(i))}for(let e of this.previous)r.has(e)||n.removeProperty(e);return this.previous=r,_}}),xn=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Sn=class extends y{constructor(){super(...arguments),this.variant=`text`,this.animation=`pulse`,this.width=``,this.height=``}static{this.styles=_n}render(){return h`<span class="skeleton" part="skeleton" style=${bn({width:this.width||void 0,height:this.height||void 0})} aria-hidden="true"></span>`}};xn([x({reflect:!0})],Sn.prototype,`variant`,void 0),xn([x({reflect:!0})],Sn.prototype,`animation`,void 0),xn([x()],Sn.prototype,`width`,void 0),xn([x()],Sn.prototype,`height`,void 0),Sn=xn([b(`lit-material-skeleton`)],Sn);var Cn=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},wn=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}};function Tn(e,t,n,r){if(!e||!(t in e))return;let i=r.getRootNode(),a=(n??``).split(/\s+/).filter(Boolean).map(e=>i.getElementById?.(e)??null).filter(e=>e!==null);e[t]=a.length?a:null}function En(e,t,n){return!e&&!t?v:h`<span class="control-text ${n?`disabled`:``}" part="text">
    ${e?h`<label for="control" part="label">${e}</label>`:v}
    ${t?h`<span class="supporting" id="supporting" part="supporting-text">${t}</span>`:v}
  </span>`}var Dn=o`
  .control-text {
    margin-inline-start: 4px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-height: 40px;
    padding-block: 10px;
    box-sizing: border-box;
  }
  .control-text label {
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-body-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-large-size, 1rem);
    line-height: var(--md-sys-typescale-body-large-line-height, 1.5rem);
    cursor: pointer;
  }
  .control-text .supporting {
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
  }
  .control-text.disabled {
    opacity: 0.38;
  }
  .control-text.disabled label {
    cursor: default;
  }
`,On=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
  }

  :host([hidden]) {
    display: none;
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
    /* Its children are absolute: without this a tight row shrinks the box under the label. */
    flex: none;
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
    outline: var(--lit-material-focus-ring-width, 3px) solid var(--lit-material-focus-ring-color, var(--md-sys-color-secondary, #625b71));
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

  /* The .focus-ring element is the one focus indicator: no second, browser-drawn ring on the inner control. */
  :where(button, a, input, [tabindex]):focus-visible {
    outline: none;
  }

  /* Touch: at least a 48px (44px for calendar days) target, the visual size unchanged where possible. */
  @media (pointer: coarse) {
    .native-control { inset: -4px 0; width: auto; height: auto; }
  }
`,A=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},j=class extends y{static{this.styles=[On,Dn]}static{this.shadowRootOptions={...y.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.checked=!1,this.disabled=!1,this.error=!1,this.required=!1,this.name=``,this.value=`on`,this.label=``,this.supportingText=``,this.ariaLabel=null,this.ariaLabelledBy=null,this.ripple=new Cn(this),this.focusRing=new wn(this),this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement),this.syncInternals()}updated(e){Tn(this.renderRoot.querySelector(`.native-control`),`ariaLabelledByElements`,this.ariaLabelledBy,this),(e.has(`checked`)||e.has(`required`)||e.has(`error`))&&this.syncInternals()}syncInternals(){if(this.internals.setFormValue(this.checked?this.value:null),this.inputElement){let e=this.error?this.inputElement.validationMessage||`Invalid`:this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){return h`
      <div class="switch ${this.checked?`checked`:``}" part="switch">
        <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
        <div class="track" part="track">
          <div class="thumb-wrap">
            <div class="state-layer" part="ripple"></div>
            <div class="thumb" part="thumb">
              ${this.checked?h`<slot name="selected-icon" class="icon"></slot>`:h`<slot name="icon" class="icon"></slot>`}
            </div>
          </div>
        </div>
        <input
          id="control"
          aria-describedby=${this.supportingText?`supporting`:v}
          class="native-control"
          part="input"
          type="checkbox"
          role="switch"
          .checked=${this.checked}
          ?disabled=${this.disabled}
          ?required=${this.required}
          name=${this.name||v}
          value=${this.value||v}
          aria-invalid=${this.error?`true`:v}
          aria-label=${this.ariaLabel||v}
          @change=${this.handleChange}
        />
      </div>
      ${En(this.label,this.supportingText,this.disabled)}
    `}handleChange(e){let t=e.target;this.checked=t.checked,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}formResetCallback(){this.checked=this.hasAttribute(`checked`),this.syncInternals()}formStateRestoreCallback(e){this.checked=e!==null}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};A([x({type:Boolean})],j.prototype,`checked`,void 0),A([x({type:Boolean,reflect:!0})],j.prototype,`disabled`,void 0),A([x({type:Boolean,reflect:!0})],j.prototype,`error`,void 0),A([x({type:Boolean})],j.prototype,`required`,void 0),A([x()],j.prototype,`name`,void 0),A([x()],j.prototype,`value`,void 0),A([x({reflect:!0})],j.prototype,`form`,void 0),A([x()],j.prototype,`label`,void 0),A([x({attribute:`supporting-text`})],j.prototype,`supportingText`,void 0),A([x({attribute:`aria-label`})],j.prototype,`ariaLabel`,void 0),A([x({attribute:`aria-labelledby`})],j.prototype,`ariaLabelledBy`,void 0),A([C(`.native-control`)],j.prototype,`inputElement`,void 0),A([C(`.state-layer`)],j.prototype,`stateLayerElement`,void 0),j=A([b(`lit-material-switch`)],j);var kn=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}};o`
  .control-text {
    margin-inline-start: 4px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-height: 40px;
    padding-block: 10px;
    box-sizing: border-box;
  }
  .control-text label {
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-body-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-large-size, 1rem);
    line-height: var(--md-sys-typescale-body-large-line-height, 1.5rem);
    cursor: pointer;
  }
  .control-text .supporting {
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
  }
  .control-text.disabled {
    opacity: 0.38;
  }
  .control-text.disabled label {
    cursor: default;
  }
`;var An=new Map,jn=new Set,Mn=`en`,Nn;function Pn(){for(let e of jn)e.refresh()}var Fn=new Map,In=new Map;function Ln(e,t,n){let r=e.get(t);if(!r){try{r=n(t)}catch{r=n(Mn)}e.set(t,r)}return r}function Rn(e,t){let n=e.toLowerCase(),r=An.get(n)?.[t];if(r!==void 0)return r;let i=n.split(`-`)[0];return i===n?void 0:An.get(i)?.[t]}function zn(e,t,n,r={}){let i=Rn(e,t)??n,a;if(typeof i==`string`)a=i;else{let t=Number(r.count??0);a=i[Ln(Fn,e,e=>new Intl.PluralRules(e)).select(t)]??i.other}return a.includes(`{`)?a.replace(/\{(\w+)\}/g,(t,n)=>{let i=r[n];return i===void 0?t:typeof i==`number`?Ln(In,e,e=>new Intl.NumberFormat(e)).format(i):i}):a}function Bn(e){let t=e;for(;t;){let e=t.closest(`[lang]`);if(e){let n=e.getAttribute(`lang`);if(n)return n;t=e.parentElement;continue}let n=t.getRootNode();t=n instanceof ShadowRoot?n.host:null}return Mn}function Vn(e){try{return e.matches(`:dir(rtl)`)}catch{return getComputedStyle(e).direction===`rtl`}}var Hn=class{constructor(e){this.host=e,e.addController(this)}hostConnected(){this.cachedLang=void 0,jn.add(this),!Nn&&typeof MutationObserver<`u`&&(Nn=new MutationObserver(()=>Pn()),Nn.observe(document.documentElement,{attributes:!0,attributeFilter:[`lang`,`dir`]}))}hostDisconnected(){jn.delete(this)}hostUpdated(){this.cachedLang=void 0}refresh(){this.cachedLang=void 0,this.host.requestUpdate()}get lang(){return this.cachedLang??=Bn(this.host)}get dir(){return Vn(this.host)?`rtl`:`ltr`}term(e,t,n){return zn(this.lang,e,t,n)}number(e,t){try{return new Intl.NumberFormat(this.lang,t).format(e)}catch{return new Intl.NumberFormat(Mn,t).format(e)}}date(e,t){try{return new Intl.DateTimeFormat(this.lang,t).format(e)}catch{return new Intl.DateTimeFormat(Mn,t).format(e)}}},Un=o`
  :host {
    display: inline-flex;
    min-width: 0;
    max-width: 100%;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
    --notch-color: var(--lit-material-text-field-notch-color, var(--md-sys-color-surface, #fffbfe));
  }

  :host([hidden]) {
    display: none;
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
    height: var(--md-sys-density-field-height, 56px);
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
    top: var(--md-sys-density-field-label-top, 8px);
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
    padding-bottom: var(--md-sys-density-field-value-inset, 8px);
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
    overflow-wrap: anywhere;
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
`,M=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Wn=0,N=class extends y{static{this.styles=Un}static{this.shadowRootOptions={...y.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.type=`text`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.prefix=``,this.suffix=``,this.step=``,this.pattern=``,this.autocomplete=``,this.inputmode=``,this.name=``,this.focused=!1,this.touched=!1,this.focusRing=new kn(this),this.localize=new Hn(this),this.labelId=`lit-material-text-field-label-${++Wn}`,this.supportingId=`lit-material-text-field-supporting-${Wn}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`pattern`)||e.has(`minlength`)||e.has(`maxlength`)||e.has(`min`)||e.has(`max`)||e.has(`type`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.inputElement?!1:!this.inputElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.inputElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.inputElement){let e=this.isError&&this.errorText||this.inputElement.validationMessage;this.internals.setValidity(this.inputElement.validity,e,this.inputElement)}}render(){let e=this.floated,t=this.isError;return h`
      <div class="text-field" part="text-field">
        <div
          class="container ${e?`floated`:``} ${this.focused?`focused`:``}"
          part="container"
        >
          <slot name="leading-icon" class="leading-icon"></slot>
          <div class="field">
            ${this.prefix?h`<span class="prefix">${this.prefix}</span>`:v}
            <div class="input-wrapper">
              ${this.label?h`<label class="label" part="label" id=${this.labelId} for="input">${this.label}</label>`:v}
              <input
                class="input"
                part="input"
                id="input"
                .value=${this.value}
                type=${this.type||`text`}
                placeholder=${this.placeholder||v}
                ?disabled=${this.disabled}
                ?readonly=${this.readonly}
                ?required=${this.required}
                autocomplete=${this.autocomplete||v}
                inputmode=${this.inputmode||v}
                min=${this.min??v}
                max=${this.max??v}
                minlength=${this.minlength??v}
                maxlength=${this.maxlength??v}
                step=${this.step||v}
                pattern=${this.pattern||v}
                name=${this.name||v}
                aria-labelledby=${this.label?this.labelId:v}
                aria-describedby=${this.supportingMessage?this.supportingId:v}
                aria-invalid=${t?`true`:v}
                role=${this.inputAria?.role??v}
                aria-valuenow=${this.inputAria?.valuenow??v}
                aria-valuemin=${this.inputAria?.valuemin??v}
                aria-valuemax=${this.inputAria?.valuemax??v}
                aria-valuetext=${this.inputAria?.valuetext??v}
                @input=${this.handleInput}
                @change=${this.handleChange}
                @focus=${this.handleFocus}
                @blur=${this.handleBlur}
                @keydown=${this.handleKeydown}
              />
            </div>
            ${this.suffix?h`<span class="suffix">${this.suffix}</span>`:v}
          </div>
          <slot name="trailing-icon" class="trailing-icon"></slot>
        </div>
        ${this.supportingMessage||this.maxlength!=null?h`
              <div class="supporting" part="supporting">
                <span class="supporting-text" id=${this.supportingId}>
                  ${this.supportingMessage}
                </span>
                ${this.maxlength==null?v:h`<span class="counter"
                      >${this.localize.term(`textField.characterCount`,`{count} / {max}`,{count:this.value.length,max:this.maxlength})}</span
                    >`}
              </div>
            `:v}
      </div>
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleKeydown(e){if(e.key!==`Enter`||e.isComposing||e.defaultPrevented)return;let t=this.internals.form;t&&(e.preventDefault(),t.requestSubmit())}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};M([x()],N.prototype,`variant`,void 0),M([x()],N.prototype,`type`,void 0),M([x()],N.prototype,`value`,void 0),M([x()],N.prototype,`label`,void 0),M([x()],N.prototype,`placeholder`,void 0),M([x({attribute:`supporting-text`})],N.prototype,`supportingText`,void 0),M([x({attribute:`error-text`})],N.prototype,`errorText`,void 0),M([x({type:Boolean,reflect:!0})],N.prototype,`error`,void 0),M([x({type:Boolean,reflect:!0})],N.prototype,`disabled`,void 0),M([x({type:Boolean})],N.prototype,`required`,void 0),M([x({type:Boolean})],N.prototype,`readonly`,void 0),M([x()],N.prototype,`prefix`,void 0),M([x()],N.prototype,`suffix`,void 0),M([x({type:Number})],N.prototype,`min`,void 0),M([x({type:Number})],N.prototype,`max`,void 0),M([x({type:Number})],N.prototype,`minlength`,void 0),M([x({type:Number})],N.prototype,`maxlength`,void 0),M([x()],N.prototype,`step`,void 0),M([x()],N.prototype,`pattern`,void 0),M([x()],N.prototype,`autocomplete`,void 0),M([x()],N.prototype,`inputmode`,void 0),M([x({attribute:!1})],N.prototype,`inputAria`,void 0),M([x()],N.prototype,`name`,void 0),M([x({reflect:!0})],N.prototype,`form`,void 0),M([C(`.input`)],N.prototype,`inputElement`,void 0),M([S()],N.prototype,`focused`,void 0),M([S()],N.prototype,`touched`,void 0),N=M([b(`lit-material-text-field`)],N);var Gn=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
    --notch-color: var(--lit-material-text-field-notch-color, var(--md-sys-color-surface, #fffbfe));
  }

  :host([hidden]) {
    display: none;
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
`,Kn=e=>e.startsWith(`--`)?e:e.replace(/[A-Z]/g,e=>`-${e.toLowerCase()}`),qn=e=>e!=null&&e!==``,Jn=tn(class extends nn{constructor(e){if(super(e),this.previous=new Set,e.type!==E.ATTRIBUTE||e.name!==`style`||(e.strings?.length??0)>2)throw Error("cssProps() must be the whole value of a style attribute: style=${cssProps({...})}")}render(e){return Object.entries(e).filter(([,e])=>qn(e)).map(([e,t])=>`${Kn(e)}: ${t}`).join(`; `)}update(e,[t]){let{style:n}=e.element,r=new Set;for(let[e,i]of Object.entries(t)){if(!qn(i))continue;let t=Kn(e);r.add(t),n.setProperty(t,String(i))}for(let e of this.previous)r.has(e)||n.removeProperty(e);return this.previous=r,_}}),P=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Yn=0,F=class extends y{static{this.styles=Gn}static{this.shadowRootOptions={...y.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`filled`,this.value=``,this.label=``,this.placeholder=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.readonly=!1,this.rows=4,this.resize=`vertical`,this.autocomplete=``,this.name=``,this.focused=!1,this.touched=!1,this.localize=new Hn(this),this.labelId=`lit-material-textarea-label-${++Yn}`,this.supportingId=`lit-material-textarea-supporting-${Yn}`,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`required`)||e.has(`minlength`)||e.has(`maxlength`))&&this.syncInternals()}get hasValue(){return this.value!==``}get floated(){return this.focused||this.hasValue||this.placeholder!==``}get isError(){return this.error?!0:!this.touched||!this.textareaElement?!1:!this.textareaElement.validity.valid}get supportingMessage(){return this.isError?this.errorText||this.textareaElement?.validationMessage||``:this.supportingText}syncInternals(){if(this.internals.setFormValue(this.value),this.textareaElement){let e=this.isError&&this.errorText||this.textareaElement.validationMessage;this.internals.setValidity(this.textareaElement.validity,e,this.textareaElement)}}render(){let e=this.floated,t=this.isError;return h`
      <div class="textarea-field" part="textarea-field">
        <div class="container ${e?`floated`:``} ${this.focused?`focused`:``}" part="container">
          ${this.label?h`<label class="label" part="label" id=${this.labelId} for="textarea">${this.label}</label>`:v}
          <textarea
            class="textarea"
            part="textarea"
            id="textarea"
            style=${Jn({resize:this.resize})}
            rows=${this.rows}
            .value=${this.value}
            placeholder=${this.placeholder||v}
            ?disabled=${this.disabled}
            ?readonly=${this.readonly}
            ?required=${this.required}
            autocomplete=${this.autocomplete||v}
            minlength=${this.minlength??v}
            maxlength=${this.maxlength??v}
            name=${this.name||v}
            aria-labelledby=${this.label?this.labelId:v}
            aria-describedby=${this.supportingMessage?this.supportingId:v}
            aria-invalid=${t?`true`:v}
            @input=${this.handleInput}
            @change=${this.handleChange}
            @focus=${this.handleFocus}
            @blur=${this.handleBlur}
          ></textarea>
        </div>
        ${this.supportingMessage||this.maxlength!=null?h`
              <div class="supporting" part="supporting">
                <span class="supporting-text" id=${this.supportingId}> ${this.supportingMessage} </span>
                ${this.maxlength==null?v:h`<span class="counter"
                      >${this.localize.term(`textField.characterCount`,`{count} / {max}`,{count:this.value.length,max:this.maxlength})}</span
                    >`}
              </div>
            `:v}
      </div>
    `}handleInput(e){this.value=e.target.value}handleChange(e){this.touched=!0,this.syncInternals(),this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0})),e.stopPropagation()}handleFocus(){this.focused=!0}handleBlur(){this.focused=!1,this.touched=!0,this.syncInternals()}formResetCallback(){this.value=this.getAttribute(`value`)??``,this.touched=!1,this.syncInternals()}formStateRestoreCallback(e){this.value=e??``}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};P([x()],F.prototype,`variant`,void 0),P([x()],F.prototype,`value`,void 0),P([x()],F.prototype,`label`,void 0),P([x()],F.prototype,`placeholder`,void 0),P([x({attribute:`supporting-text`})],F.prototype,`supportingText`,void 0),P([x({attribute:`error-text`})],F.prototype,`errorText`,void 0),P([x({type:Boolean,reflect:!0})],F.prototype,`error`,void 0),P([x({type:Boolean,reflect:!0})],F.prototype,`disabled`,void 0),P([x({type:Boolean})],F.prototype,`required`,void 0),P([x({type:Boolean})],F.prototype,`readonly`,void 0),P([x({type:Number})],F.prototype,`minlength`,void 0),P([x({type:Number})],F.prototype,`maxlength`,void 0),P([x({type:Number})],F.prototype,`rows`,void 0),P([x()],F.prototype,`resize`,void 0),P([x()],F.prototype,`autocomplete`,void 0),P([x()],F.prototype,`name`,void 0),P([x({reflect:!0})],F.prototype,`form`,void 0),P([C(`.textarea`)],F.prototype,`textareaElement`,void 0),P([S()],F.prototype,`focused`,void 0),P([S()],F.prototype,`touched`,void 0),F=P([b(`lit-material-textarea`)],F);var Xn=class{constructor(e){this.surface=null,this.handlePointerDown=e=>{e.button!==0||this.isDisabled()||this.surface?.setAttribute(`data-pressed`,``)},this.handlePointerEnd=()=>{this.surface?.removeAttribute(`data-pressed`)},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`pointerup`,this.handlePointerEnd),this.host.addEventListener(`pointercancel`,this.handlePointerEnd),this.host.addEventListener(`pointerleave`,this.handlePointerEnd)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`pointerup`,this.handlePointerEnd),this.host.removeEventListener(`pointercancel`,this.handlePointerEnd),this.host.removeEventListener(`pointerleave`,this.handlePointerEnd)}attach(e){this.surface=e}isDisabled(){return`disabled`in this.host&&!!this.host.disabled}},Zn=class{constructor(e){this.visible=!1,this.pointerActive=!1,this.handlePointerDown=()=>{this.pointerActive=!0,this.visible&&(this.visible=!1,this.host.requestUpdate())},this.handleFocusIn=()=>{this.visible=!this.pointerActive,this.pointerActive=!1,this.host.requestUpdate()},this.handleFocusOut=()=>{this.visible=!1,this.pointerActive=!1,this.host.requestUpdate()},this.host=e,e.addController(this)}hostConnected(){this.host.addEventListener(`pointerdown`,this.handlePointerDown),this.host.addEventListener(`focusin`,this.handleFocusIn),this.host.addEventListener(`focusout`,this.handleFocusOut)}hostDisconnected(){this.host.removeEventListener(`pointerdown`,this.handlePointerDown),this.host.removeEventListener(`focusin`,this.handleFocusIn),this.host.removeEventListener(`focusout`,this.handleFocusOut)}};function Qn(e,t,n,r){if(!e||!(t in e))return;let i=r.getRootNode(),a=(n??``).split(/\s+/).filter(Boolean).map(e=>i.getElementById?.(e)??null).filter(e=>e!==null);e[t]=a.length?a:null}o`
  .control-text {
    margin-inline-start: 4px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-height: 40px;
    padding-block: 10px;
    box-sizing: border-box;
  }
  .control-text label {
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-body-large-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-large-size, 1rem);
    line-height: var(--md-sys-typescale-body-large-line-height, 1.5rem);
    cursor: pointer;
  }
  .control-text .supporting {
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
    line-height: var(--md-sys-typescale-body-medium-line-height, 1.25rem);
  }
  .control-text.disabled {
    opacity: 0.38;
  }
  .control-text.disabled label {
    cursor: default;
  }
`;var $n=o`
  :host {
    display: inline-flex;
    outline: none;
    vertical-align: top;
    -webkit-tap-highlight-color: transparent;
  }

  :host([hidden]) {
    display: none;
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
    width: var(--md-sys-density-icon-button-size, 40px);
    height: var(--md-sys-density-icon-button-size, 40px);
    padding: var(--md-sys-density-icon-button-padding, 8px);
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
    outline: var(--lit-material-focus-ring-width, 3px) solid var(--lit-material-focus-ring-color, var(--md-sys-color-secondary, #625b71));
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
    color: color-mix(in srgb, var(--md-sys-color-on-surface, #1c1b1f) var(--lit-material-disabled-text-mix, 50%), transparent);
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

  /* The .focus-ring element is the one focus indicator: no second, browser-drawn ring on the inner control. */
  :where(button, a, input, [tabindex]):focus-visible {
    outline: none;
  }

  /* Touch: at least a 48px (44px for calendar days) target, the visual size unchanged where possible. */
  @media (pointer: coarse) {
    .button::after { content: ""; position: absolute; inset: -4px; }
  }
`,I=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},L=class extends y{static{this.styles=$n}static{this.shadowRootOptions={...y.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.variant=`standard`,this.toggle=!1,this.selected=!1,this.disabled=!1,this.type=`button`,this.name=``,this.value=``,this.href=``,this.target=``,this.ariaLabel=null,this.ariaLabelledBy=null,this.ripple=new Xn(this),this.focusRing=new Zn(this),this.internals=this.attachInternals()}firstUpdated(){this.stateLayerElement&&this.ripple.attach(this.stateLayerElement)}updated(){Qn(this.renderRoot.querySelector(`.button`),`ariaLabelledByElements`,this.ariaLabelledBy,this)}render(){let e=this.toggle&&this.selected?h`<slot name="selected-icon" class="icon"></slot>`:h`<slot name="icon" class="icon"></slot><slot class="icon"></slot>`,t=h`
      <div class="focus-ring" part="focus-ring" ?hidden=${!this.focusRing.visible}></div>
      <div class="state-layer" part="ripple"></div>
      ${e}
    `;return this.href?h`
        <a
          class="button"
          part="button"
          href=${this.disabled?v:this.href}
          target=${this.target||v}
          rel=${this.target===`_blank`?`noopener noreferrer`:v}
          aria-disabled=${this.disabled?`true`:v}
          aria-label=${this.ariaLabel||v}
          tabindex=${this.disabled?`-1`:`0`}
          @click=${this.handleLinkClick}
        >
          ${t}
        </a>
      `:h`
      <button
        class="button"
        part="button"
        type="button"
        ?disabled=${this.disabled}
        name=${this.name||v}
        value=${this.value||v}
        aria-pressed=${this.toggle?this.selected:v}
        aria-label=${this.ariaLabel||v}
        @click=${this.handleClick}
      >
        ${t}
      </button>
    `}handleClick(e){this.disabled?(e.preventDefault(),e.stopImmediatePropagation()):this.toggle?(this.selected=!this.selected,this.dispatchEvent(new Event(`change`,{bubbles:!0}))):this.type===`submit`?this.internals.form?.requestSubmit():this.type===`reset`&&this.internals.form?.reset()}handleLinkClick(e){this.disabled&&(e.preventDefault(),e.stopImmediatePropagation())}};I([x()],L.prototype,`variant`,void 0),I([x({type:Boolean,reflect:!0})],L.prototype,`toggle`,void 0),I([x({type:Boolean,reflect:!0})],L.prototype,`selected`,void 0),I([x({type:Boolean,reflect:!0})],L.prototype,`disabled`,void 0),I([x()],L.prototype,`type`,void 0),I([x()],L.prototype,`name`,void 0),I([x()],L.prototype,`value`,void 0),I([x({reflect:!0})],L.prototype,`form`,void 0),I([x()],L.prototype,`href`,void 0),I([x()],L.prototype,`target`,void 0),I([x({attribute:`aria-label`})],L.prototype,`ariaLabel`,void 0),I([x({attribute:`aria-labelledby`})],L.prototype,`ariaLabelledBy`,void 0),I([C(`.state-layer`)],L.prototype,`stateLayerElement`,void 0),L=I([b(`lit-material-icon-button`)],L);var R=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},er=h`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19 13H5v-2h14v2z"></path></svg>`,tr=h`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"></path></svg>`;function nr(e){let t=String(e),n=/e-(\d+)$/.exec(t);return n?Number(n[1]):t.includes(`.`)?t.length-t.indexOf(`.`)-1:0}var z=class extends y{constructor(){super(...arguments),this.value=null,this.step=1,this.label=``,this.unit=``,this.variant=`outlined`,this.supportingText=``,this.errorText=``,this.name=``,this.required=!1,this.disabled=!1,this.readonly=!1,this.localize=new Hn(this),this.validationMessage=``,this.draft=null}static{this.styles=o`
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
  `}checkValidity(){return this.validationMessage=this.check(),!this.validationMessage}check(){return this.draft!==null&&this.draft.trim()!==``?this.localize.term(`textField.enterANumber`,`Enter a number`):this.value===null?this.required?this.localize.term(`textField.required`,`Required`):``:this.min!==void 0&&this.value<this.min?this.localize.term(`textField.atLeast`,`At least {min}`,{min:this.min}):this.max!==void 0&&this.value>this.max?this.localize.term(`textField.atMost`,`At most {max}`,{max:this.max}):``}willUpdate(e){e.has(`value`)&&(this.draft=null)}update(e){this.validationMessage=this.check(),super.update(e)}clamp(e){return this.min!==void 0&&(e=Math.max(this.min,e)),this.max!==void 0&&(e=Math.min(this.max,e)),e}stepBy(e){if(this.value===null){this.commit(this.clamp(this.min??0));return}let t=this.value,n=Math.max(nr(this.step),nr(t));this.commit(this.clamp(Number((t+e*this.step).toFixed(n))))}commit(e){let t=e!==this.value||this.draft!==null;this.value=e,this.draft=null,t&&this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0}))}onInput(e){e.stopPropagation();let t=e.target.value.trim(),n=t===``?null:Number(t.replace(`,`,`.`));n!==null&&!Number.isFinite(n)?(this.draft=t,this.validationMessage=this.check()):(this.value=n,this.draft=null,this.dispatchEvent(new Event(`input`,{bubbles:!0,composed:!0})))}onKeyDown(e){if(this.disabled||this.readonly)return;let t={ArrowUp:1,ArrowDown:-1,PageUp:10,PageDown:-10};e.key in t?(e.preventDefault(),this.stepBy(t[e.key])):e.key===`Home`&&this.min!==void 0?(e.preventDefault(),this.commit(this.min)):e.key===`End`&&this.max!==void 0&&(e.preventDefault(),this.commit(this.max))}render(){let e=this.disabled||this.readonly,t=this.validationMessage||this.errorText,n=this.draft??(this.value===null?``:String(this.value));return h`
      <lit-material-icon-button
        part="decrement"
        aria-label=${this.label?this.localize.term(`textField.decreaseLabel`,`Decrease {label}`,{label:this.label}):this.localize.term(`textField.decreaseValue`,`Decrease value`)}
        ?disabled=${e||this.min!==void 0&&this.value!==null&&this.value<=this.min}
        @click=${()=>this.stepBy(-1)}
        ><span slot="icon">${er}</span></lit-material-icon-button
      >
      <lit-material-text-field
        part="field"
        variant=${this.variant}
        label=${this.label}
        inputmode=${this.step%1==0&&(this.min??0)>=0?`numeric`:`decimal`}
        autocomplete="off"
        .inputAria=${{role:`spinbutton`,valuenow:this.value??void 0,valuemin:this.min,valuemax:this.max,valuetext:this.value===null?void 0:this.unit?`${this.value} ${this.unit}`:void 0}}
        .value=${n}
        suffix=${this.unit||v}
        name=${this.name||v}
        supporting-text=${this.supportingText}
        error-text=${t}
        ?error=${!!t}
        ?required=${this.required}
        ?disabled=${this.disabled}
        ?readonly=${this.readonly}
        @input=${this.onInput}
        @change=${e=>{e.stopPropagation(),this.draft===null&&this.dispatchEvent(new Event(`change`,{bubbles:!0,composed:!0}))}}
        @keydown=${this.onKeyDown}
      ></lit-material-text-field>
      <lit-material-icon-button
        part="increment"
        aria-label=${this.label?this.localize.term(`textField.increaseLabel`,`Increase {label}`,{label:this.label}):this.localize.term(`textField.increaseValue`,`Increase value`)}
        ?disabled=${e||this.max!==void 0&&this.value!==null&&this.value>=this.max}
        @click=${()=>this.stepBy(1)}
        ><span slot="icon">${tr}</span></lit-material-icon-button
      >
    `}};R([x({type:Number})],z.prototype,`value`,void 0),R([x({type:Number})],z.prototype,`min`,void 0),R([x({type:Number})],z.prototype,`max`,void 0),R([x({type:Number})],z.prototype,`step`,void 0),R([x()],z.prototype,`label`,void 0),R([x()],z.prototype,`unit`,void 0),R([x()],z.prototype,`variant`,void 0),R([x({attribute:`supporting-text`})],z.prototype,`supportingText`,void 0),R([x({attribute:`error-text`})],z.prototype,`errorText`,void 0),R([x()],z.prototype,`name`,void 0),R([x({type:Boolean})],z.prototype,`required`,void 0),R([x({type:Boolean,reflect:!0})],z.prototype,`disabled`,void 0),R([x({type:Boolean})],z.prototype,`readonly`,void 0),R([S()],z.prototype,`validationMessage`,void 0),R([S()],z.prototype,`draft`,void 0),z=R([b(`lit-material-number-field`)],z);var rr=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ir=class extends y{static{this.styles=o`
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
  `}willUpdate(){this.hasAttribute(`role`)||this.setAttribute(`role`,`group`)}render(){return h`<slot></slot>`}};ir=rr([b(`lit-material-input-group`)],ir);var B=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},ar=0,V=class extends y{static{this.styles=o`
    :host {
      display: inline-block;
      font-family: var(--md-sys-typescale-body-large-font, inherit);
      color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    }
    :host([hidden]) {
      display: none;
    }
    label {
      display: block;
      margin-block-end: 8px;
      font-size: var(--md-sys-typescale-body-medium-size, 0.875rem);
      color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    }
    .field {
      position: relative;
      width: fit-content;
    }
    .cells {
      display: flex;
      gap: 8px;
    }
    .cell {
      display: grid;
      place-items: center;
      width: 44px;
      height: 52px;
      box-sizing: border-box;
      border: 1px solid var(--md-sys-color-outline, light-dark(#79747e, #938f99));
      border-radius: var(--md-sys-shape-corner-small, 8px);
      font-family: var(--lit-material-one-time-code-font, ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace);
      font-size: 1.5rem;
      font-variant-numeric: tabular-nums;
    }
    .cell.gap {
      margin-inline-start: 12px;
    }
    :host(:focus-within) .cell.active {
      border: 2px solid var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
    }
    :host([error]) .cell,
    .invalid .cell {
      border-color: var(--md-sys-color-error, light-dark(#b3261e, #f2b8b5));
    }
    :host([disabled]) {
      opacity: 0.38;
    }
    /* The real input covers the boxes, its own text and caret invisible. */
    input {
      position: absolute;
      inset: 0;
      width: 100%;
      height: 100%;
      box-sizing: border-box;
      padding: 0;
      border: 0;
      margin: 0;
      opacity: 0;
      font-size: 16px; /* no zoom on focus in iOS Safari */
      color: transparent;
      caret-color: transparent;
      background: transparent;
      cursor: text;
    }
    .supporting {
      margin-block-start: 4px;
      font-size: var(--md-sys-typescale-body-small-size, 0.75rem);
      color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    }
    :host([error]) .supporting {
      color: var(--md-sys-color-error, light-dark(#b3261e, #f2b8b5));
    }
  `}static{this.shadowRootOptions={...y.shadowRootOptions,delegatesFocus:!0}}static{this.formAssociated=!0}constructor(){super(),this.value=``,this.length=6,this.alphanumeric=!1,this.groupSize=0,this.label=``,this.supportingText=``,this.errorText=``,this.error=!1,this.disabled=!1,this.required=!1,this.name=``,this.caret=0,this.localize=new Hn(this),this.ids=++ar,this.internals=this.attachInternals()}connectedCallback(){super.connectedCallback(),this.syncInternals()}firstUpdated(){this.syncInternals()}updated(e){(e.has(`value`)||e.has(`length`)||e.has(`required`))&&this.syncInternals()}clean(e){let t=this.alphanumeric?/[^0-9A-Z]/g:/[^0-9]/g;return e.toUpperCase().replace(t,``).slice(0,this.length)}syncInternals(){this.internals.setFormValue(this.value);let e=this.value.length>0&&this.value.length<this.length,t=this.required&&this.value===``;if(e||t){let e=t?this.localize.term(`oneTimeCode.required`,`Enter the code`):this.localize.term(`oneTimeCode.incomplete`,`Enter all {length} characters`,{length:this.length}),n=t?{valueMissing:!0}:{tooShort:!0};this.inputElement?this.internals.setValidity(n,e,this.inputElement):this.internals.setValidity(n,e)}else this.internals.setValidity({})}render(){let e=[...this.value],t=Math.min(this.caret,this.length-1),n=this.error?this.errorText:this.supportingText,r=`lit-material-one-time-code-label-${this.ids}`,i=`lit-material-one-time-code-supporting-${this.ids}`,a=this.label||this.localize.term(`oneTimeCode.label`,`Code`);return h`
      ${this.label?h`<label id=${r} for="input">${this.label}</label>`:v}
      <div class="field">
        <div class="cells" part="cells" aria-hidden="true">
          ${Array.from({length:this.length},(n,r)=>h`<span
              part="cell"
              class="cell ${r===t?`active`:``} ${this.groupSize>0&&r>0&&r%this.groupSize===0?`gap`:``}"
              >${e[r]??``}</span
            >`)}
        </div>
        <input
          id="input"
          .value=${this.value}
          ?disabled=${this.disabled}
          ?required=${this.required}
          name=${this.name||v}
          autocomplete="one-time-code"
          inputmode=${this.alphanumeric?`text`:`numeric`}
          autocapitalize=${this.alphanumeric?`characters`:v}
          spellcheck="false"
          maxlength=${this.length*2}
          aria-label=${this.label?v:a}
          aria-labelledby=${this.label?r:v}
          aria-describedby=${n?i:v}
          aria-invalid=${this.error?`true`:v}
          @input=${this.handleInput}
          @keydown=${this.handleKeydown}
          @keyup=${this.trackCaret}
          @click=${this.trackCaret}
          @focus=${this.trackCaret}
          @select=${this.trackCaret}
        />
      </div>
      ${n?h`<div class="supporting" part="supporting" id=${i}>${n}</div>`:v}
    `}handleInput(e){let t=e.target,n=this.value,r=this.clean(t.value);t.value!==r&&(t.value=r),this.value=r,this.caret=r.length,this.syncInternals(),r.length===this.length&&n!==r&&this.dispatchEvent(new CustomEvent(`complete`,{detail:{value:r},bubbles:!0,composed:!0}))}handleKeydown(e){if(e.key!==`Enter`||e.isComposing)return;let t=this.internals.form;t&&(e.preventDefault(),t.requestSubmit())}trackCaret(){this.caret=this.inputElement?.selectionStart??this.value.length}clear(){this.value=``,this.caret=0,this.focus()}formResetCallback(){this.value=``,this.caret=0}formDisabledCallback(e){this.disabled=e}checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}};B([x()],V.prototype,`value`,void 0),B([x({type:Number})],V.prototype,`length`,void 0),B([x({type:Boolean})],V.prototype,`alphanumeric`,void 0),B([x({type:Number,attribute:`group-size`})],V.prototype,`groupSize`,void 0),B([x()],V.prototype,`label`,void 0),B([x({attribute:`supporting-text`})],V.prototype,`supportingText`,void 0),B([x({attribute:`error-text`})],V.prototype,`errorText`,void 0),B([x({type:Boolean,reflect:!0})],V.prototype,`error`,void 0),B([x({type:Boolean,reflect:!0})],V.prototype,`disabled`,void 0),B([x({type:Boolean})],V.prototype,`required`,void 0),B([x()],V.prototype,`name`,void 0),B([C(`input`)],V.prototype,`inputElement`,void 0),B([S()],V.prototype,`caret`,void 0),V=B([b(`lit-material-one-time-code`)],V);var or=new Map,sr=new Set,cr=`en`,lr;function ur(){for(let e of sr)e.refresh()}var dr=new Map,fr=new Map;function pr(e,t,n){let r=e.get(t);if(!r){try{r=n(t)}catch{r=n(cr)}e.set(t,r)}return r}function mr(e,t){let n=e.toLowerCase(),r=or.get(n)?.[t];if(r!==void 0)return r;let i=n.split(`-`)[0];return i===n?void 0:or.get(i)?.[t]}function hr(e,t,n,r={}){let i=mr(e,t)??n,a;if(typeof i==`string`)a=i;else{let t=Number(r.count??0);a=i[pr(dr,e,e=>new Intl.PluralRules(e)).select(t)]??i.other}return a.includes(`{`)?a.replace(/\{(\w+)\}/g,(t,n)=>{let i=r[n];return i===void 0?t:typeof i==`number`?pr(fr,e,e=>new Intl.NumberFormat(e)).format(i):i}):a}function gr(e){let t=e;for(;t;){let e=t.closest(`[lang]`);if(e){let n=e.getAttribute(`lang`);if(n)return n;t=e.parentElement;continue}let n=t.getRootNode();t=n instanceof ShadowRoot?n.host:null}return cr}function _r(e){try{return e.matches(`:dir(rtl)`)}catch{return getComputedStyle(e).direction===`rtl`}}var vr=class{constructor(e){this.host=e,e.addController(this)}hostConnected(){this.cachedLang=void 0,sr.add(this),!lr&&typeof MutationObserver<`u`&&(lr=new MutationObserver(()=>ur()),lr.observe(document.documentElement,{attributes:!0,attributeFilter:[`lang`,`dir`]}))}hostDisconnected(){sr.delete(this)}hostUpdated(){this.cachedLang=void 0}refresh(){this.cachedLang=void 0,this.host.requestUpdate()}get lang(){return this.cachedLang??=gr(this.host)}get dir(){return _r(this.host)?`rtl`:`ltr`}term(e,t,n){return hr(this.lang,e,t,n)}number(e,t){try{return new Intl.NumberFormat(this.lang,t).format(e)}catch{return new Intl.NumberFormat(cr,t).format(e)}}date(e,t){try{return new Intl.DateTimeFormat(this.lang,t).format(e)}catch{return new Intl.DateTimeFormat(cr,t).format(e)}}},yr=/[\s/._:\-?&=>›()[\]]/;function br(e,t){let n=e.toLowerCase().replace(/\s+/g,``);if(!n)return{score:0,at:[]};let r=t.toLowerCase(),i=null;for(let e=r.indexOf(n[0]);e!==-1;e=r.indexOf(n[0],e+1)){let a=[e],o=e;for(let e=1;e<n.length&&(o=r.indexOf(n[e],o+1),o!==-1);e++)a.push(o);if(a.length<n.length)break;let s=0;for(let e=0;e<a.length;e++){let n=a[e];s+=1,(n===0||yr.test(t[n-1]))&&(s+=3),e>0&&n===a[e-1]+1&&(s+=4)}s-=e*.05,(!i||s>i.score)&&(i={score:s,at:a})}return i}var xr=o`
  :host {
    display: contents;
  }

  dialog {
    box-sizing: border-box;
    width: var(--lit-material-command-palette-width, min(640px, 100vw - 32px));
    max-width: none;
    margin: var(--lit-material-command-palette-top, 12vh) auto auto;
    padding: 0;
    border: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    border-radius: var(--md-sys-shape-corner-extra-large, 16px);
    background: var(--md-sys-color-surface-container-high, light-dark(#ece6f0, #2b2930));
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: 0.875rem;
    line-height: 1.25rem;
    box-shadow: 0 24px 80px rgb(0 0 0 / 0.45);
    overflow: hidden;
  }
  dialog:has(.with-preview) {
    width: var(--lit-material-command-palette-preview-width, min(960px, 100vw - 32px));
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.5);
  }

  .field {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 56px;
    padding: 0 16px;
    border-bottom: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
  }
  .search {
    flex: none;
    width: 18px;
    height: 18px;
    fill: none;
    stroke: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    stroke-width: 1.8;
    stroke-linecap: round;
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    outline: 0;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 1rem;
  }
  input::placeholder {
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
  }

  .body {
    display: flex;
    min-height: 0;
  }
  .list {
    flex: 1;
    min-width: 0;
    max-height: var(--lit-material-command-palette-max-height, min(60vh, 420px));
    overflow-y: auto;
    padding: 6px;
  }
  .preview {
    flex: 0 0 52%;
    min-width: 0;
    max-height: var(--lit-material-command-palette-max-height, min(60vh, 420px));
    overflow: auto;
    padding: 12px 16px;
    border-inline-start: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    background: var(--md-sys-color-surface-container, light-dark(#f3edf7, #211f26));
  }
  @media (max-width: 719.98px) {
    .preview {
      display: none;
    }
    dialog:has(.with-preview) {
      width: var(--lit-material-command-palette-width, min(640px, 100vw - 32px));
    }
  }
  .method {
    flex: none;
    min-width: 3.25em;
  }
  .section {
    padding: 10px 10px 4px;
    font-size: 0.6875rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
  }
  .option {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 40px;
    padding: 0 10px;
    border-radius: var(--md-sys-shape-corner-small, 8px);
    cursor: pointer;
  }
  .option[aria-selected="true"] {
    background: var(--md-sys-color-surface-container-highest, light-dark(#e6e0e9, #36343b));
    box-shadow: inset 2px 0 0 var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
  }
  .option[aria-disabled="true"] {
    opacity: 0.5;
    cursor: default;
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  mark {
    background: none;
    color: var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
    font-weight: 600;
  }
  .hint {
    flex: none;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    font-size: 0.75rem;
  }
  .keys {
    display: inline-flex;
    gap: 4px;
  }
  kbd {
    display: inline-block;
    min-width: 1.4em;
    padding: 0 5px;
    border: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    border-bottom-width: 2px;
    border-radius: 5px;
    font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
    font-size: 0.6875rem;
    line-height: 1.1rem;
    text-align: center;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
  }
  .empty {
    padding: 24px 10px;
    text-align: center;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
  }
  .footer {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    padding: 8px 16px;
    border-top: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    font-size: 0.75rem;
    color: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
  }
  .footer kbd {
    margin-inline-end: 2px;
  }

  @media (prefers-reduced-motion: no-preference) {
    dialog[open] {
      animation: enter 120ms ease-out;
    }
    @keyframes enter {
      from {
        opacity: 0;
        transform: translateY(-4px);
      }
    }
  }
`,Sr=o`
  :host {
    display: inline-flex;
    vertical-align: middle;
  }

  :host([hidden]) {
    display: none;
  }

  .tag {
    font-family: var(--lit-material-method-tag-font, ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace);
    font-size: 0.6875rem;
    line-height: 1rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  :host([size="medium"]) .tag {
    font-size: 0.8125rem;
    line-height: 1.25rem;
  }

  .get {
    color: var(--lit-material-method-tag-get, light-dark(#2f6b4a, #5ec79b));
  }
  .post {
    color: var(--lit-material-method-tag-post, light-dark(#8a5a12, #f2b45c));
  }
  .put {
    color: var(--lit-material-method-tag-put, light-dark(#2457a8, #6ca8ff));
  }
  .patch {
    color: var(--lit-material-method-tag-patch, light-dark(#6a3fb5, #c69cff));
  }
  .delete {
    color: var(--lit-material-method-tag-delete, light-dark(#a3221b, #ff8a8a));
  }
  .other {
    color: var(--lit-material-method-tag-other, var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff)));
  }
`,Cr=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},wr=[`get`,`post`,`put`,`patch`,`delete`],Tr=e=>{let t=e.trim().toLowerCase();return wr.includes(t)?t:`other`},Er=class extends y{constructor(){super(...arguments),this.method=``,this.size=`small`}static{this.styles=Sr}render(){return h`<span class="tag ${Tr(this.method)}" part="tag">${this.method.trim().toUpperCase()}</span>`}};Cr([x()],Er.prototype,`method`,void 0),Cr([x({reflect:!0})],Er.prototype,`size`,void 0),Er=Cr([b(`lit-material-method-tag`)],Er);var Dr=new Set,Or;function kr(){if(typeof document>`u`)return;for(let e of Dr)e instanceof HTMLDialogElement&&(!e.isConnected||!e.open)&&Dr.delete(e);let e=document.documentElement;Dr.size>0&&!Or?(Or={overflow:e.style.overflow,gutter:e.style.scrollbarGutter},e.scrollHeight>e.clientHeight&&(e.style.scrollbarGutter=`stable`),e.style.overflow=`hidden`):Dr.size===0&&Or&&(e.style.overflow=Or.overflow,e.style.scrollbarGutter=Or.gutter,Or=void 0)}function Ar(e){Dr.add(e),kr()}function jr(e){Dr.delete(e),kr()}function Mr(e){e.showModal(),Ar(e),e.addEventListener(`close`,()=>jr(e),{once:!0})}var H=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Nr=0;function Pr(){let e=document.activeElement;for(;e?.shadowRoot?.activeElement;)e=e.shadowRoot.activeElement;return e}var U=class extends y{constructor(){super(...arguments),this.open=!1,this.items=[],this.placeholder=``,this.label=``,this.prefixes={},this.hints=[],this.query=``,this.active=0,this.localize=new vr(this),this.uid=`lmcp${++Nr}`,this.returnFocus=null}static{this.styles=xr}show(){this.open=!0}hide(){this.open=!1}parse(){for(let[e,t]of Object.entries(this.prefixes))if(e&&this.query.startsWith(e))return{section:t,text:this.query.slice(e.length).trim()};return{section:null,text:this.query.trim()}}groups(){let{section:e,text:t}=this.parse(),n=[],r=new Map;return this.items.forEach((i,a)=>{let o=i.section??``;if(e!==null&&o!==e)return;let s=br(t,i.label);if(!s&&i.keywords?.length){let e=i.keywords.map(e=>br(t,e)).filter(e=>e!==null).sort((e,t)=>t.score-e.score)[0];e&&(s={score:e.score-1,at:[]})}s&&(r.has(o)||(r.set(o,[]),n.push(o)),r.get(o).push({item:i,index:a,score:s.score,at:s.at}))}),n.map(e=>{let n=r.get(e);return t&&n.sort((e,t)=>t.score-e.score||e.index-t.index),{section:e,rows:n}})}rows(){return this.groups().flatMap(e=>e.rows)}optionId(e){return`${this.uid}-o${e.index}`}willUpdate(e){if(e.has(`open`)&&this.open&&(this.query=``,this.active=0),e.has(`open`)||e.has(`items`)||e.has(`query`)){let e=this.rows();(this.active>=e.length||e[this.active]?.item.disabled)&&(this.active=this.firstEnabled(e,0,1))}}updated(e){let t=this.dialogElement;if(e.has(`open`)&&t){if(this.open&&!t.open)this.returnFocus=Pr(),Mr(t),this.inputElement?.focus();else if(!this.open&&t.open){t.close();let e=this.returnFocus;this.returnFocus=null,e?.isConnected&&e.focus()}}this.open&&this.renderRoot.querySelector(`[role="option"][aria-selected="true"]`)?.scrollIntoView?.({block:`nearest`})}firstEnabled(e,t,n){for(let r=t;r>=0&&r<e.length;r+=n)if(!e[r].item.disabled)return r;return e.findIndex(e=>!e.item.disabled)===-1?0:this.active}dismiss(){this.open&&(this.open=!1,this.dispatchEvent(new Event(`close`,{bubbles:!0,composed:!0})))}choose(e){if(!e||e.item.disabled)return;let t={item:e.item,query:this.query};this.dispatchEvent(new CustomEvent(`select`,{detail:t,bubbles:!0,composed:!0})),this.dismiss()}onKeyDown(e){let t=this.rows(),n=t=>{e.preventDefault(),this.active=t};switch(e.key){case`ArrowDown`:return n(this.firstEnabled(t,this.active+1,1));case`ArrowUp`:return n(this.firstEnabled(t,this.active-1,-1));case`Home`:return e.ctrlKey||e.metaKey||!this.query?n(this.firstEnabled(t,0,1)):void 0;case`End`:return e.ctrlKey||e.metaKey||!this.query?n(this.firstEnabled(t,t.length-1,-1)):void 0;case`Enter`:return e.preventDefault(),this.choose(t[this.active]);case`Escape`:return e.preventDefault(),this.dismiss();case`Tab`:if(e.preventDefault(),!e.shiftKey){let e={query:this.query};this.dispatchEvent(new CustomEvent(`tab`,{detail:e,bubbles:!0,composed:!0,cancelable:!0}))}return}}onCancel(e){e.preventDefault(),this.dismiss()}onDialogClick(e){e.target===this.dialogElement&&this.dismiss()}highlight(e){let t=e.item.label;if(!e.at.length)return t;let n=new Set(e.at),r=[],i=``,a=!1;for(let e=0;e<t.length;e++){let o=n.has(e);o!==a&&i&&(r.push(a?h`<mark>${i}</mark>`:i),i=``),a=o,i+=t[e]}return i&&r.push(a?h`<mark>${i}</mark>`:i),r}render(){let e=this.open?this.groups():[],t=e.flatMap(e=>e.rows),n=t[this.active],r=`${this.uid}-list`,i=this.open&&n&&this.preview?this.preview(n.item):void 0,a=i!=null&&i!==v,o=this.label||this.localize.term(`commandPalette.label`,`Command palette`);return h`
      <dialog part="dialog" aria-label=${o} @cancel=${this.onCancel} @click=${this.onDialogClick}>
        <div class="panel ${a?`with-preview`:``}">
          <div class="field">
            <svg class="search" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7"></circle><path d="M20 20l-3.5-3.5"></path></svg>
            <input
              part="input"
              type="text"
              role="combobox"
              autocomplete="off"
              spellcheck="false"
              aria-label=${o}
              aria-expanded=${this.open&&t.length?`true`:`false`}
              aria-controls=${r}
              aria-autocomplete="list"
              aria-activedescendant=${n?this.optionId(n):v}
              placeholder=${this.placeholder||this.localize.term(`commandPalette.placeholder`,`Search or run a command…`)}
              .value=${this.query}
              @input=${e=>{this.query=e.target.value,this.active=0}}
              @keydown=${this.onKeyDown}
            />
          </div>
          <div class="body">
          <div class="list" id=${r} role="listbox" part="listbox" aria-label=${this.localize.term(`commandPalette.results`,`Results`)}>
            ${e.map(e=>h`<div role="group" class="group" aria-label=${e.section||v}>
                ${e.section?h`<div class="section" aria-hidden="true">${e.section}</div>`:v}
                ${e.rows.map(e=>{let n=t.indexOf(e);return h`<div
                    class="option"
                    part="option"
                    role="option"
                    id=${this.optionId(e)}
                    aria-selected=${n===this.active?`true`:`false`}
                    aria-disabled=${e.item.disabled?`true`:v}
                    @mousemove=${()=>{!e.item.disabled&&this.active!==n&&(this.active=n)}}
                    @click=${()=>this.choose(e)}
                  >
                    ${e.item.method?h`<lit-material-method-tag class="method" part="method" method=${e.item.method}></lit-material-method-tag>`:v}
                    <span class="label">${this.highlight(e)}</span>
                    ${e.item.hint?h`<span class="hint">${e.item.hint}</span>`:v}
                    ${e.item.shortcut?.length?h`<span class="keys">${e.item.shortcut.map(e=>h`<kbd>${e}</kbd>`)}</span>`:v}
                  </div>`})}
              </div>`)}
            ${this.open&&!t.length?h`<div class="empty" role="status">${this.localize.term(`commandPalette.empty`,`No results`)}</div>`:v}
          </div>
          ${a?h`<div class="preview" part="preview">${i}</div>`:v}
          </div>
          <div class="footer" part="footer" aria-hidden="true">
            <span><kbd>↑</kbd><kbd>↓</kbd> ${this.localize.term(`commandPalette.move`,`move`)}</span>
            <span><kbd>↵</kbd> ${this.localize.term(`commandPalette.choose`,`choose`)}</span>
            <span><kbd>esc</kbd> ${this.localize.term(`commandPalette.dismiss`,`close`)}</span>
            ${this.hints.map(e=>h`<span>${e.keys.map(e=>h`<kbd>${e}</kbd>`)} ${e.label}</span>`)}
          </div>
        </div>
      </dialog>
    `}};H([x({type:Boolean,reflect:!0})],U.prototype,`open`,void 0),H([x({type:Array})],U.prototype,`items`,void 0),H([x()],U.prototype,`placeholder`,void 0),H([x()],U.prototype,`label`,void 0),H([x({type:Object})],U.prototype,`prefixes`,void 0),H([x({attribute:!1})],U.prototype,`preview`,void 0),H([x({type:Array})],U.prototype,`hints`,void 0),H([S()],U.prototype,`query`,void 0),H([S()],U.prototype,`active`,void 0),H([C(`dialog`)],U.prototype,`dialogElement`,void 0),H([C(`input`)],U.prototype,`inputElement`,void 0),U=H([b(`lit-material-command-palette`)],U);var Fr=`rws-admin-token`,Ir=``;function Lr(){try{return sessionStorage.getItem(Fr)??Ir}catch{return Ir}}function Rr(e){Ir=e;try{e?sessionStorage.setItem(Fr,e):sessionStorage.removeItem(Fr)}catch{}}var W=class extends Error{constructor(e,t){super(t),this.status=e}};async function G(e,{method:t=`GET`,body:n}={}){let r=await fetch(`/admin/api/v1${e}`,{method:t,headers:{...Lr()?{Authorization:`Bearer ${Lr()}`}:{},"X-Rws-Admin":`1`,...n===void 0?{}:{"Content-Type":`application/json`}},credentials:`same-origin`,body:n===void 0?void 0:JSON.stringify(n),cache:`no-store`}),i=await r.text(),a=null;try{a=i?JSON.parse(i):null}catch{a=null}if(!r.ok)throw new W(r.status,a?.error??`${r.status} ${r.statusText}`);return a}async function zr(e,t){let n=await fetch(`/admin/api/v1/logs/stream`,{headers:{...Lr()?{Authorization:`Bearer ${Lr()}`}:{},"X-Rws-Admin":`1`},credentials:`same-origin`,signal:t,cache:`no-store`});if(!n.ok||!n.body)throw new W(n.status,`${n.status} ${n.statusText}`);let r=n.body.pipeThrough(new TextDecoderStream).getReader(),i=``;for(;;){let{value:t,done:n}=await r.read();if(n)return;i+=t;let a;for(;(a=i.indexOf(`

`))>=0;){let t=i.slice(0,a);if(i=i.slice(a+2),t.split(`
`).some(e=>e.startsWith(`event:`)))continue;let n=t.split(`
`).filter(e=>e.startsWith(`data:`)).map(e=>e.slice(5).replace(/^ /,``)).join(`
`);n&&e(n)}}}var Br=[`B`,`KiB`,`MiB`,`GiB`,`TiB`,`PiB`,`EiB`],Vr=[`B`,`kB`,`MB`,`GB`,`TB`,`PB`,`EB`];function Hr(e,t={}){let{locale:n,maximumFractionDigits:r=1,...i}=t;return new Intl.NumberFormat(n,{maximumFractionDigits:r,...i}).format(e)}function Ur(e,t={}){if(!Number.isFinite(e))return``;let n=t.binary===!1?1e3:1024,r=t.binary===!1?Vr:Br,i=Math.abs(e),a=0;for(;i>=n&&a<r.length-1;)i/=n,a++;let o=t.maximumFractionDigits??1;return a<r.length-1&&Number(i.toFixed(o))>=n&&(i/=n,a++),`${e<0?`-`:``}${Hr(i,{...t,maximumFractionDigits:a===0?0:o})} ${r[a]}`}var Wr=[[`day`,864e5],[`hour`,36e5],[`minute`,6e4],[`second`,1e3],[`millisecond`,1]];function Gr(e,t={}){if(!Number.isFinite(e))return``;let{locale:n,largest:r=2,unitDisplay:i=`short`}=t,a=Math.abs(Math.round(e)),o=[];for(let[t,s]of Wr){if(t===`millisecond`&&(o.length||Math.abs(e)>=1e3))break;let c=Math.floor(a/s);if(a-=c*s,c>0&&o.push(new Intl.NumberFormat(n,{style:`unit`,unit:t,unitDisplay:i}).format(c)),o.length===r)break}o.length===0&&o.push(new Intl.NumberFormat(n,{style:`unit`,unit:`second`,unitDisplay:i}).format(0));let s=new Intl.ListFormat(n,{style:i,type:`unit`}).format(o);return e<0?`-${s}`:s}var{I:Kr}=We,qr=e=>e.strings===void 0,Jr={},Yr=(e,t=Jr)=>e._$AH=t,Xr=tn(class extends nn{constructor(e){if(super(e),e.type!==E.PROPERTY&&e.type!==E.ATTRIBUTE&&e.type!==E.BOOLEAN_ATTRIBUTE)throw Error("The `live` directive is not allowed on child or event bindings");if(!qr(e))throw Error("`live` bindings can only contain a single expression")}render(e){return e}update(e,[t]){if(t===_||t===v)return t;let n=e.element,r=e.name;if(e.type===E.PROPERTY){if(t===n[r])return _}else if(e.type===E.BOOLEAN_ATTRIBUTE){if(!!t===n.hasAttribute(r))return _}else if(e.type===E.ATTRIBUTE&&n.getAttribute(r)===t+``)return _;return Yr(e),t}}),Zr=new Map,Qr=new Set,$r=`en`,ei;function ti(){for(let e of Qr)e.refresh()}var ni=new Map,ri=new Map;function ii(e,t,n){let r=e.get(t);if(!r){try{r=n(t)}catch{r=n($r)}e.set(t,r)}return r}function ai(e,t){let n=e.toLowerCase(),r=Zr.get(n)?.[t];if(r!==void 0)return r;let i=n.split(`-`)[0];return i===n?void 0:Zr.get(i)?.[t]}function oi(e,t,n,r={}){let i=ai(e,t)??n,a;if(typeof i==`string`)a=i;else{let t=Number(r.count??0);a=i[ii(ni,e,e=>new Intl.PluralRules(e)).select(t)]??i.other}return a.includes(`{`)?a.replace(/\{(\w+)\}/g,(t,n)=>{let i=r[n];return i===void 0?t:typeof i==`number`?ii(ri,e,e=>new Intl.NumberFormat(e)).format(i):i}):a}function si(e){let t=e;for(;t;){let e=t.closest(`[lang]`);if(e){let n=e.getAttribute(`lang`);if(n)return n;t=e.parentElement;continue}let n=t.getRootNode();t=n instanceof ShadowRoot?n.host:null}return $r}function ci(e){try{return e.matches(`:dir(rtl)`)}catch{return getComputedStyle(e).direction===`rtl`}}var li=class{constructor(e){this.host=e,e.addController(this)}hostConnected(){this.cachedLang=void 0,Qr.add(this),!ei&&typeof MutationObserver<`u`&&(ei=new MutationObserver(()=>ti()),ei.observe(document.documentElement,{attributes:!0,attributeFilter:[`lang`,`dir`]}))}hostDisconnected(){Qr.delete(this)}hostUpdated(){this.cachedLang=void 0}refresh(){this.cachedLang=void 0,this.host.requestUpdate()}get lang(){return this.cachedLang??=si(this.host)}get dir(){return ci(this.host)?`rtl`:`ltr`}term(e,t,n){return oi(this.lang,e,t,n)}number(e,t){try{return new Intl.NumberFormat(this.lang,t).format(e)}catch{return new Intl.NumberFormat($r,t).format(e)}}date(e,t){try{return new Intl.DateTimeFormat(this.lang,t).format(e)}catch{return new Intl.DateTimeFormat($r,t).format(e)}}},ui=e=>e.startsWith(`--`)?e:e.replace(/[A-Z]/g,e=>`-${e.toLowerCase()}`),di=e=>e!=null&&e!==``,fi=tn(class extends nn{constructor(e){if(super(e),this.previous=new Set,e.type!==E.ATTRIBUTE||e.name!==`style`||(e.strings?.length??0)>2)throw Error("cssProps() must be the whole value of a style attribute: style=${cssProps({...})}")}render(e){return Object.entries(e).filter(([,e])=>di(e)).map(([e,t])=>`${ui(e)}: ${t}`).join(`; `)}update(e,[t]){let{style:n}=e.element,r=new Set;for(let[e,i]of Object.entries(t)){if(!di(i))continue;let t=ui(e);r.add(t),n.setProperty(t,String(i))}for(let e of this.previous)r.has(e)||n.removeProperty(e);return this.previous=r,_}}),pi=`0 0 1000 439`,mi={AE:`M637 154l1 0 0 1 2-1 2 0 2 0 1-2 2-2 1-2 1 1 1 3-2 0 0 2 1 0-1 1 0 2 0 1 0 2-1 0-8-1-2-4 0-1z`,AF:`M666 109l2 0 2 1 1 0 1-1 1 1 1-2 1 0 0-2 1-1 2 1 0 1 1 3 1 1 1-1 1 0 1-2 2 1 2 0 1 1-1 0-1 1-3 0-2 1-1 1 1 2 0 1 0 2 0 1 0 1-3 0 2 2-2 1 0 2 1 2-1 1-1 0-2 0 0 1-2 0-1 2 0 3-3 2-1-1-1 1-1 0-3 0-5-2 2-3-1-2-2-1 0-2-2-2 1-2-2-1 1-2 0-4 2 1 2-1 0-1 2-1 1-1 0-2 2-1 0-1 2 1z`,AL:`M551 98l1 0-1 1 0 1-1 1-1 0-1-2-1-1 0-2 1-1-1-1 0-1 1-1 1 0 0 1 1 0 0 1 0 1 0 2 1 1z`,AM:`M615 104l-1 0-1-2-1 0-1-1-1 0-1-1-2-1 0-1 0-1 3-1 1 1 1 1-1 0 2 2-1 1 2 0 1 1 0 2z`,AO:`M536 261l-1 1 0 1-1 2-1-3 1-1 1-1 1 1zM534 266l1 0 1 0 1-1 8 0 1 3 1 2 0 2 1 2 2-1 1 0 2 0 0-1 1-2 1 0 1-1 1 0 0 2 3 0 0 2 1 1-1 3 0 2 1 1 0 5 1-1 1 0 2 0 1 0 0 1 0 2 0 2 0 1 0 1-6 0 0 12 1 3 2 2-5 2-7-1-1-1-11 0-1 0-1-2-2 0-2 1-1 1 0-3 0-3 1-3 0-2 1-3 1-2 2-2 0-2 1-3 0-2-1-1-1-2 0-2 0-1 1-2-1-3-1-3-1-2 0-1z`,AR:`M346 425l2 1 2 2 3 2 4 1 0 1-3 0-1-1-1 0-3 0-3-6zM350 354l0 2 0 4 1 3 0 1 0 2 0 2 4 3 0 2 2 2 1 1-2 5-3 1-5 1-3 0 2 2 0 2 1 2-1 1-3 0-3-1 0 1 1 3 2 1 1-1 1 2-2 1-1 2 1 3 0 1-2 0-2 2 1 2 3 2 2 1 1 3-3 1 0 4-1 1 0 1 2 3 2 2-1 0-2-1-6 0-2-2-1-2-1 0-1-1-2-3 1-1 0-2-1-1 0-3-1-4-1-2 1 0-1-2-1 0 0-1-1-2-2-3 1-1-2-4-1-3 0-3 1-1-2-3-1-4 2-2-1-2 0-4-1-3-3-6 1-4-1-3 0-3 1-4 1-2-1-1 0-1-1-6 3-2 0-4 0-1 1-3 4 1 2 2 0-2 4 0 0 1 6 5 2 1 4 3 3 1 0 1-1 6 3 1 3 0 2 0 2-3 0-3 1-1 1 2 1 3-2 2-2 2-2 3-3 5z`,AT:`M539 75l0 1-1 0 1 1-1 2-2 0-2 1-1 0-4-1-1-1-2 1-1 0-2 0-1-1 1-1-1 0 1 0 1 0 1 0 2 0 1-1 1 0 1 1 0-1 0-2 1 0 0-1 2 1 1-2 1 0 2 1 1 0 1 1 0 1z`,AU:`M862 389l1 0-3 4-1 1-2 3-1-1-3 2-2 0 1-3 1-2 1-3 1-1 1 0 2 1 1 0 3-1zM824 361l-3 1-3 1-1 2-2 1-2 0-2 1-2-1-2 0-1 1-3 1-2 1-2 1-2 0-1 0-2-2-1-1 1-2 1 0 1-1 0-1 2-3 0-2 0-3 1-2 1-2-1-2 1-1-1-2 1-2-1-3 0-2 1 2 0-3 1 1 1 1 0-2 0-3 0-1-1-1 1-2 1-1 1-2 0-2 2-2 0 2 1-2 3-1 2-2 2-1 1 0 1 0 3-1 1-1 1 0 1-1 1 0 3-1 2-1 1-2 2-2 1-2 0-2 3-3 0 3 2 0-1-2 1-2 1 1 1-3 2-2 1-1 2-1 0-1 1 1 0-1 2-1 1 0 2 1 1 3 2 0 1 0 0-2 2-3 1-1 0-1 2-2 2-1 1 0 2 0 1-2-2-2 1 0 2 1 1 1 2 1 1 0 2 1 2-1 1 0 1 1-1 2-1 2-1 0 0 1-1 2-2 2 0 1 2 2 2 1 1 2 1 2 1 0 1 1 0 1 3 1 2-1 1-2 1-2 1-2 1-3 0-1 1-2 0-2 1-2 0-1 0-1 1-2 1-2 0-1 2-2 0 2 0 2 0 1 0 1 1 2 0 2-1 2 1 2 2-1 0 2 1 1 0 2-1 3 0 2 1 0 0 3-1 2 0 3 2 2 2 1 1 2-1 1 1 2 0 4 1-1 1 2 1-1-1 4 1 2 0 1 1 3 0 3-1 2-1 2-1 3-1 3-2 2-2 3-1 2-2 3-3 3-3 1-2 3-2 2-2 3-2 1-3 3-1 2-1 1-2 1-3 0-3 2-2 1-3 1-1-1-1-1 2-1-2 0-4 3-1-1-1-1-2 0-2-1 0-2 1-3 1-1-1-2-2 0 2-2 0-2-2 2-3 1 3-2 1-2 2-2 1-2-3 3-3 1-2 3-1-2 1-2-1-2 0-1 1-1-3-2-1 0-2-2-5 0-4 2-3 1-3 0z`,AZ:`M613 94l1 0 2 2 1 1 0-1 1-2 2 2 2 2 1 1 1 0-2 1 0 2 0 1-1 1 1 2-1 0-2-2 1-1-1-1-1 0-3 2 0-2-1-1-2 0 1-1-2-2 1 0-1-1-1-1 2 1 2 0 0-1-2-1 1-1zM614 104l-1 0-2-2-1-1 1 0 1 1 1 0 1 2z`,BA:`M545 92l-2-1-1-2-1 0-2-2 0-1-1-1 0-1 1 0 1 0 3 0 1 0 1 1 1 0 0 1 1 1 0 2-1 0-1 1 0 2z`,BD:`M748 162l1 3-1-1 1 3-2-2 0-2-1-2-2-2-2 0 1 2-1 2-1-1 0 1-1-1-1 0-1-3-2-3 0-2-2-1 0-1 2-1-3-2 1-3 2 2 1 0 1 2 3 1 2 0 2 0 0 3-2 1 0 2 2 2 0-3 1 0 2 6z`,BE:`M514 67l0 2-1 0 0 2-2-2-1 1-2-2-1-1-1 0 0-1 2 0 1 0 2-1 2 1 1 1z`,BF:`M485 205l0-2 1-2 0-1 2-3 0-3 1-1 1 1 1-1 1-1 2-2 1-1 2-1 2-1 0 1 2 0 0 2 0 1 2 3 0 1 3 1 0 3-1 1-1 0-1 2-1 0-2 0-1 0-1 1-1-1-5 0 0 3 0 2-2-1-1 1-1 0-1 0-1-2-1 0z`,BG:`M554 87l1 1 1 0 2 0 3 1 2-1 2-1 2 1 2 0-1 2-1 2 1 2-2-1-2 1 0 2-2 0-2-1-2 1-2 0 0-2-2-1 1-1-1 0 1-1 0-1-1-1 0-1 0-1z`,BI:`M585 252l0 2 0 1 0 1 0 1-1 2-1 1-2 0 0-4 0-2 1 0 1-2 2 0z`,BJ:`M507 220l-2 0-1-2 1-9-1 0 0-2-1-2-1-1 0-2 1 0 1-2 1 0 1-1 1-1 1 0 2 2 0 1 0 2 0 2 0 1-1 2-1 1 0 3 0 2-1 6z`,BN:`M820 223l0 2 0 2-1 0-1 1-1-2 1-1 2-2z`,BO:`M309 284l2 1 1-1 1-1 2-1 1-2 4 0 0 3 0 1 0 2 3 4 3 0 2 2 2 1 1 1 2 0 1 1 1 2 0 1 0 1 0 1 1 4 5 0 0 2 1 1 1 2 1 2 0 3-1 1 1 2-1 1 0-1-3-2-2 0-5 1-1 3 1 2-1 5 0-1-4 0 0 2-2-2-4-1-1 3-2 1-2-5-2-4 0-4-2-1 0-3-2-2 1-4-1-3 0-2 0-1 0-2 0-3 0-3 0-1-2-6z`,BR:`M364 366l-1-2 1-2-2-2-3-2-3-2-1 0-3-3-2 1 3-5 2-3 2-2 2-2-1-3-1-2-1 1 0-2 0-2-1-2-1-1-1 1-1 0 0-2-1-3 0-1-2-1-1 1-3-1-1-5-1-2 1-1-1-2 1-1 0-3-1-2-1-2-1-1 0-2-5 0-1-4 0-1 0-1 0-1-1-2-1-1-2 0-1-1-2-1-2-2-3 0-3-4 0-2 0-1 0-3-4 0-1 2-2 1-1 1-1 1-2-1-2 1-1-1-1-5-2 2-2 0-1-2-2 0 0-2-1-2-2-4 1 0 0-2 2-1-1-2 1-1 0-2 3-2 2-1 1-1 2 0 1-10 0-1 0-3-2-1 1-3 1 0 1 0 0-1-2-1 0-2 6 0 0-1 1 1 1 2 2 2 2-1 0-1 3-1 1 0 0-2 2-1 0-1-2 0-1-2 0-3-1-1 1 0 2 0 2 1 1-1 2 0 3-2 1-1 0-1 1 0 1 1-1 1 1 1 1 2-1 1-1 3 1 2 0 2 2 1 1 1 1-1 1 0 1-1 1-1 1 0 1 0 1 1 1-1-1-1 1-1 1 0 1 0 2 1 1 0 1-1 0 1 1 1 1-1 1-1 1-3 2-3 1 0 0 2 2 6 1 1 0 2-2 4 1 1 5 0 0 4 2-2 4 1 4 2 2 2-1 2 3-1 6 2 4 0 4 3 4 4 2 1 2 1 1 1 1 5 1 2-1 6-1 2-4 6-2 4-1 3-1 0-1 3 1 7 0 5-1 3 0 1 0 5-3 5 0 3-2 2 0 2-3 0-4 1-2 2-3 1-2 3-2 3 0 3 1 2 0 4-1 1-1 2-2 6-2 3-1 2-1 3-1 2z`,BS:`M292 145l1 0 2 0 0 1-3 0 0-1zM295 144l2 2-1 2 0-2-1-1 0-1zM293 151l1 0 0 3-1 2-1-2 0-1 1-2z`,BT:`M741 142l1 1 0 2-2 0-2 0-2 0-2-1-1-1 1-2 2-1 2 1 1 0 2 1z`,BW:`M579 325l-4 3-3 2-1 3-1 1-1 0-1 2 0 1-2 1-2 0-1-1-1-1-2 1 0 2-2 1-1 1-2 1 0-1 0-3-1-3-1 0 0-11 3 0 1-13 2 0 4-1 1 1 2-1 1 0 1-1 1 0 0 3 1 1 1 2 3 4 1 0 0 1 0 3 2 0 2 2z`,BY:`M561 52l3 1 0 1 1-1 2 1 1 1 0 1 1 2 1 0 0 1 2 0 1 1-1 0-2 0 0 1 1 1 1 1-2 1-1 0 0 2-1-1-2 0-1 0-1 0-2 0-3-1-2 0-2 0-1 1-1 0 0-1-1-2 1 0 0-1-1-1 0-2 2 0 2-1 1-1 1-1 0-1 1-1 2-1z`,BZ:`M258 177l1 0 1-2 1 0 0 1-1 2 0 2-1 2 0 1-1 0-1 1 1-4 0-3z`,CA:`M216 72l-2-3-1-1-3-1 1-3 2-1-2-1 2-3-1-2 1-1 2-1 2-2-2-2 1-3 1-2-1-1 0-1 0-1-3 1-4 1 0-2 0-1-1 0-2 0 17-14 10-7 3 0 2 1 2 0 2 0 4-1 2 0 4-1 4 0 0 1 2-1 2-1 1 1 1 1 4-1-2 1 3 0 1 0 3 0 1 0 4 1 2 1 2-1 2 1-4 1 3 1 5 0 2-1 1 1 3-1-1 0 2-1 3 0 1 0 1 0 1 1 2 0 3 1 3 0 3 0 1-2 2 0 2 1-1 2 2-2 2 0 3-2-1-1-2-1 3-2 3-1 2 0 1 1 1 2-3 1 3 0-1 2 3-1 2 1-2 1 0 2 4-2 3-1 2-2 2 0 3 0 1 1 0 1-3 1 1 1-1 1-5 2-3 0-2-1-1 1-4 2-1 1-4 2-3 0-2 1-1 1-3 0-4 2-4 3-3 2-2 3 3 0-1 3-1 2 4-1 3 1 1 1 1 2 2 0 2 2 3 0 3 0-2 2-1 3 0 3 2 3 2-1 3-3 1-4-1-2 5-1 3-2 2-2 1-2 0-2-1-2 4-2 0-3 2-3 2-1 3 1 2 0 2-1 2 1 1 2 0 1 4 0-1 2-1 3 1 0 1 2 4-2 3-2 2-2 1 3 1 3 1 3-2 2 2 1 2 2 3 0 1 1 0 3 1 0 1 1-1 3-2 1-2 1-4 1-4 3-4 0-5 0-3 0-3 0-3 2-3 1-5 4-4 3 2-1 6-4 5-2 4 0 1 1-3 2 0 3 0 3 2 1 4 0 4-4-1 3 2 1-4 2-6 1-3 2-3 2-2 0 1-3 5-2-4 0-3 0-1-2 2-4-1-1-1 1-1-1-3 2-1 3-2 1-1 1-1 0-5 0-3 0-2 1-3 2-2 2-2 0-2 0-2 0 1 1-1 1-4 2-2 0-4 2-1-1 1-2 2-1 1-2 1-3 1-3-2-1 1-1-1 0 0-1-1 0 0-1-2-1-1-1-2-1-1-2-3 1-1 0-2 0-2 0-2-1-2 0-1-1-1 0 1-2-1 0-1 1-5 0-7 0-8 0-7 0-7 0-7 0-7 0-2 0-7 0-7 0zM329 36l2-1 3 0-1 1-3 1-1 0 0-1zM354 15l-1-1 1 0 4 0 3 1-1 0-1 0-2 0-3 0zM335 37l2 0 1 0-2 1-1 0 0-1zM332 12l-1 0-3 0-2 0 2-1 3 0 1 0 0 1zM336 8l-1 0-3 0 3 0 1 0zM333 7l1 0-1 1-3 0-1 0 0-1 3 0 1 0zM341 13l-3 0-4-1 0-1 1 0-1-1-4 0-1-1 1 0 4 0 1 0 4 0 1 1-1 0 1 1 1 0 3 0 2 0 3 0 4 0 3 0 1 0 0 1-2 0-3 1-2-1-5 1-4 0zM306 7l3 1-1 0-4 0-2 0 2 0 2-1zM308 7l2 0-3 0-2 0 2 0 1 0zM374 66l-2 1-2 3 2-1 1 1-1 1 1 0 2 0 2 1-2 2 2-1 0 2 0 2-2 2-1 0-1 0 1-3-1 0-3 3-1 0 2-2-2-1-3 1-4 0 0-1 1-1 0-1 2-2 4-5 2-1 3-1 1 0-1 1zM333 30l2 1 2 1-1 1 2 0 1 0-3 1-2 0-1-1-3 1-4 1 1-1-3 0 3-1 1-2 3-2 1 0-1 1 2 0zM355 16l2-1 4 1 2 1-1 1 4-1 1 2 4 0 1 1 1 2-4 1 3 1 3 1 1 1 3 1-1 1-5 3-2-1-2-2-2 0-1 1 1 1 2 1 0 1 0 2-1 2-3-1-4-2 2 2 1 2 0 1-5-1-3-2-2-1 1 0-2-2-2-1-1 1-6 0-1 0 3-2 3 0 5 0 0-1 1-1 4-2 0-1 0-1-2-1-4-1 2 0-1-1-2 0-1-1-1 1-4 0-6-1-4 0-2 0-1-1 2-1-2 0 1-2 3-1 2-1 5 0-2 1 1 1 3-2 5 0 1 1-1 2 4-1zM329 13l4 0 3 1-4 1-3 0-3 2-2 0 0-2 1-1 2 0 2-1zM282 10l5-1 5-1 2 0 3 0-2 1-2 1-1 0-5 0-3 0-2 0zM206 58l3 0-4 3 0 2-1 0 0-1 0-1 0-1 1-2 1 0zM318 6l3 0 5 1 0 1-2 0-3-1-4 0 2 0-1 0 0-1zM214 74l-2 0-3-1 0-1-1-1 0-1-1-1 0-1 1-1 2 1 1 0 2 1 0 1 0 1 1 1 0 2zM281 13l2 0 5 0 1 1 1 0-3 1-6 1-4 1-1 1-6 1 0-1-3-1 2-1 3-1 3-1 0-1 6 0zM308 11l2-1 2 0 0 1-2 1-7 0-5 1-3 0 1-1 4 0-8 0-2-1 4-1 2 0 5 0 2 1 3 0-1-1 2-1 2 0-1 1 0 1zM306 15l1 1-1 1-1 1 2 1 3 1-1 1-4 0 1 1-2 1-3-1-2 0-3 0-4 1-5 0-4 0 0-1-2 0-2 0 0-2 1 0 4 0 2 0 3 0-3-1-4 0-3 0 5-1-3 0-2-1 3-1 2-1 6-1 2 1-2 1 4-1 2 1 3-1 1 1 0 1 1 0 1-2 2 0 2 0zM316 15l-1-1 3 0 2 0 3 0-2 1 2 1-2 1-4 1-2 0 0-1-3-1 0-1 4 0zM306 14l3 0 1 0-3 1-1-1zM326 9l1 1-1 1-2 1-3 0-2 0 1-1-3 0 2-1 1 0 4-1 2 1 0-1zM336 5l2-1 2 0-1 0 4 0 2 1 2 0 3 0 0 1 2 0-3 0-4 1-3 0-3 0-2 0 1-1 2 0-4 0-1 0-1-1 2 0zM345 4l3-1 2 0 3 0 3 0 2 0 2 0 1 0 3 0 3 0 5 0 1 0 5-1 3 1 4 0 4 0 4 0 3 0-5 1-4 0-2 0 4 0-5 0-3 1-3 1-4 0-1 0-6 0 3 0-2 0 1 1-2 0-3 1-1 0-3 1 3 0-6 1-4 0-5 0-3 0-3 0 0-1 4 0 0-1 1 0 4 0-1-1-3 0 2 0 3-1 1 0-2 0 0-1 5 0 1 0 3 0-4 0-6 0-2 0-1-1-2 0zM354 25l-1 1-3 0 1-1 1-1 2 0 1 0 0 1-1 0zM318 21l0 1-2 1-2-1-2 0-1-1 2 0 2-1 2 1 1 0zM352 70l1 0 3 0 1 2-1 0-2-1-2-1zM350 78l0 2 2 0 2 0-2 1-1 0-2-1 0-1 1-1z`,CD:`M581 260l1 4 0 2 0 2 2 2 1 5-1-1-4 1-1 0 0 3 0 1-1 5 0 3 1 1 2 1 0 4-2 0-1-2-1-2-2 0 0-2-2 1-2-1-1-1-2-1-1 0 0-1-1 0-1 0-2 0-1 0-1 1 0-5-1-1 0-2 1-3-1-1 0-2-3 0 0-2-1 0-1 1-1 0-1 2 0 1-2 0-1 0-2 1-1-2 0-2-1-2-1-3-8 0-1 1-1 0-1 0 0-1 1-2 0-1 1-1 1 1 1-2 1 0 0 1 1 1 2-2 2-2 0-1 0-3 2-4 1-2 2-2 0-1 0-1 1-2-1-2 1-3 0-2 1-2 0-3 0-2 2-2 1-1 2 1 2 1 2 1 2 0 1-2 2 0 3-2 1 1 1 0 0-1 1 0 2 0 2 0 1 0 2 3 1 0 1 0 1 0 1-1 1 2 3 2-1 5 2 0-1 1-1 1-1 2-1 2 0 3-1 2 0 2-1 1 0 3 0 2 0 2 0 4z`,CF:`M576 224l-1 0-2 0-2 0-1 0 0 1-1 0-1-1-3 2-2 0-1 2-2 0-2-1-2-1-2-1-1 1-2 2 0 2-2 0-1-1-2 2-2 4 0-1 0-2-1-1-1-2-1-1-1-2 0-1 0-2 0-3 1-1 1-3 3-1 0-1 1 0 0 1 4-1 1-2 1-1 0-1 1-1 2 1 3-2 2-4 1-2 2 0 0 1 2 3 0 1 0 2 0 1 1 1 2 2 1 1 1 1 1 2 2 2 0 2 2 2 1 1z`,CG:`M551 230l0 3-1 2 0 2-1 3 1 2-1 2 0 1 0 1-2 2-1 2-2 4 0 3 0 1-2 2-2 2-1-1 0-1-1 0-1 2-1-1-1-1-1 1-1 1-2-4 2-2-1-2 1-1 2-1 0-1 1 1 3 1 1-2 0-3 0-3-2-2 2-4-1-1-2 0-1-1 0-2 4 0 2 1 2 1 0-2 2-4 2-2 1 1 2 0z`,CH:`M522 77l1 0-1 1 1 1 2 0-1 1-1 0-1 0-1 1-1 0-2 1-1 0-1-1-1-1-1 1 0-2 2-2 1 0 0-1 2 0 1 0 2 1z`,CI:`M478 205l1 0 2 1 1-2 0 1 1-1 0 2 1-1 1 0 1 0 1 2 1 0 1 0 1-1 2 1 1 6-1 3-1 4 1 3 0 2-1 0-2-1-2 0-3 1-2 1-3 1 0-3 0-1 0-1-1-2-1 0-1-1 1-1 0-2 0-1 0-2 0-1 1-1 0-3-1-1 0-1 1-1z`,CL:`M346 425l3 6 3 0 1 0 0 1-2 1-1 0-1 0-2-1-3 0-3-2-3-1-5-4 2 1 4 2 4 1 0-1 0-2 1-1 2 0zM311 309l2 2 0 3 2 1 0 4 2 4 2 5 2-1 0 1 0 4-3 2 1 6 0 1 1 1-1 2-1 4 0 3 1 3-1 4 3 6 1 3 0 4 1 2-2 2 1 4 2 3-1 1 0 3 1 3 2 4-1 1 2 3 1 2 0 1 1 0 1 2-1 0 1 2 1 4 0 3 1 1 0 2-1 1 2 3 1 1 1 0 1 2 2 2 6 0 2 1-2 0-1 1-1 1 1 2-1 0-3-1-4-2-4-1-1-2 0-2-2-2-3-5 0-2 1-3-4-1 1-2-1-6 3 2-1-7-2-1 1 4-2 0-1-5-1-6 0-2-2-3-1-4 1 0 0-5 0-6 0-5-2-5 0-3-2-4 1-4-1-7-1-8 0-8-1-5-2-6 2-1 0-1z`,CM:`M540 195l1 3 0 2 0 3 2 3-2 0-1 0-1 0 0 2 1 2 1 1 1 1 1 3-1 1-1 3-1 1 0 3 0 2 0 1 1 2 1 1 1 2 1 1 0 2 0 1 0 2-2-1-2-1-4 0-2 0-1 0-2 0-4 0 0-3-1-3-1 0-1-2 0-1 0-3 2-4 2-2 1 0 2 2 2-2 0-1 0-2 1-2 1-1 1-3 0-1 1-2 0-2 3-3 0-1 0-1-1-1 0-1 1-1z`,CN:`M797 176l-3-1 0-3 0-2 3-1 2 0 1 1-1 2 0 2-2 2zM695 93l-1-2 1-1-5-5 5-2 1 0-1-6 5 1 0-1-1-3 2 0 0-2 1-1 2 3 3 1 4 1 3 3 1 3 1 2 3 0 4 1 3 1 2 1 3 3 2 2 3-1 5 1 3 0 3 0 4 2 3 0 2 1 2-2 3-1 4 0 2-1 1-2 1-1-1-1-2-1 0-2 2 0 2 1 2-2 3-1 0-2 1-1 3 0 2 0 0-1-4-2-2-1-1 1-3 0-1 0-1-1-1-3 0-2 3 1 2-2-1-1 0-3 0-1-1-2-2 0 1-2 2 0 3 0 4 0 3 1 4 3 2 2 2 1 3 3 5 1 4 2 3 3 4 0 1-1 3-1 1 3 0 1 2 3 1 3-4-1-1 2 3 2 2 4-1 0 1 2-3-2 0 1-3 2 2 2-2-1-2-1-1 3-1 1-1 2-3 1-1 2-2 1 1-2-2-1 1-2-2-2-2 1-1 3-1 2-2 0 0 1 2 3 2 0 1 2 3 1 1-3 3 1 2 1 1 1-3 1 0 2-2 2 0 2 4 2 2 3 3 3 3 3 1 3-1 0 1 2 2 1 1 3 1 3-2 0 0 4-1 5-1 4-2 3-3 3-3 1-1 1-1-1-1 2-4 2-2 0 0 4-2 0-1-2 0-2-4-1-1 1-3-1-2-2 0-2-2 0-2-2-2 2-2 1-2 0-2 0-1 1 2 4-2 0 0-1-1-1-1 1-2-1-2-1 0-3-2-1-1-3-3 0-1-4 2-3-1-3-1-2-1-1-2-2-1 0-3-1 0-1-2-2-1 1-2-1-3 3-1 2-2 1-2-1-1 0-2-1-2 1-1 2-1-3-1 1-3 0-3-1-3-1-2-1-1-2-2 0-3-3-3-1 0 1-5-2-3-2-2-4 2 0 0-1-2-2 0-3-4-4-5-1-1-3-3-1-1-1-1-2 0-1-2-1-2-3 0-1 2-2 1-1 3 1 0-2 3-1 0-1 4-2z`,CO:`M314 239l-1-2-1-1 0 1-6 0 0 2 2 1 0 1-1 0-1 0-1 3 2 1 0 3 0 1-1 10-1-2-1 0 2-3-3-2-1 0-1-1-2 1-2 0-2-4-1-1-1-2-2-1-2-1-1-1-1 0-2 0-1-2-3-2 0-1 1 0 0-2 0-1 2-1 1-2 1-2-1-1 1-2-1-4 1 0-1-4-1-2 1-1 1 0 0-1 0-2 0-1 2 0 2-3 1 0 0-1 1-3 2-2 2 0 0-1 2 0 2-1 1-1 2-2 1 0 1 1-1 1-2 1-1 2-1 1-1 1 0 3-1 2 1 0 0 2 1 1 0 1 0 2 0 1 1 1 3 0 2 0 2 4 1-1 2 0 1 0 1 1 0 2-1 1 0 3 0 2 1 1 0 1-1 2 1 1 0 1 1 4z`,CR:`M272 208l-1 0 0 1 1 1-1 0 0 1 0 1 0 1-2-1 0-1 0-1-1-1-1-1-1-1 0-1 0 1-1 1 0-1-1-1-1 0 1-2 0-1-1 0 1-1 1 0 1 1 1-1 1 0 0 1 1 0 1 0 0 2 1 1 1 2z`,CU:`M280 158l3 0 2 0 2 1 1 2 2-1 1 1 2 2 1 2 1 0 2 1-1 1 2 0 2 1 0 1-2 1-2 0-2-1-4 1 2-2-1-1-1 0-1-1 0-2-2 0-2-1-1-1-3-1-1 0 1-1-2 0-3 1-1 1-2 1-1-1 2-1 1-1 1-1 2 0 2-1z`,CY:`M583 116l1 0 1 0 0 1 1-1 0 1-2 1-2 0 0-2 1 0z`,CZ:`M534 66l1 1 2 0 0 1 1 1 1-1 1 0 0 1 2 0 1 2-1 0 0 1-1 0 0 1-1 0-1 1-1-1-1 0-2-1-1 0-1 2-2-1-1-2-1 0-1-1 0-1 2-1 0-1 2 0 0-1 1 1 1-1z`,DE:`M531 59l1 1-1 1 1 1 1 1 0 1 1 2-1 1-1-1 0 1-2 0 0 1-2 1 0 1 1 1 1 0 1 2 0 1-1 0 0 2 0 1-1-1-1 0-1 1-2 0-1 0-1 0-1 0-2-1-1 0-2 0 1-2 1-2-4 0-1-1 0-1 0-1 0-2 0-3 1 0 0-1 1-3-1 0 1-1 2 0 1-1 0-1 0-2 1 1 2-1 0 1 2 1 0 1 2 0 2-1 2 1 1 1z`,DJ:`M616 197l1 0 1-1 1 1 0 2-2 1 2 1-1 2-1-1-1 0-1 0 0-1 0-1 0-2 1-1z`,DK:`M522 55l-2 1-1-1-1-1 0-3 0-1 2 0 1-1 2-1 0 1-1 1 0 1 2 0-1 1-2 2 1 1zM527 52l1 2-1 2-3-2 0-1 3-1z`,DO:`M306 176l0-1-1-1 1 0 0-2 0-2 1 0 2 0 1 1 1 0 0 1 2 0 0 1 1 0 1 1-1 2-1-1-2 0-1 0 0 1-1 0 0-1-1 1-2 2-1-1 1-1z`,DZ:`M477 143l0-1 0-4 5-3 2 0 2-1 1-2 3-1 1-3 1 0 1-1 4-1 0-1-1-1 0-4-1-2 0-2 2-2 3 0 1-2 3-1 4 0 4 0 1 0 3-1 2 0 1 0 2 0 0 2 0 3 0 3-2 2 0 2 3 2 0 1 1 2 2 6 1 3 0 1-1 3 1 2-1 2 1 2-1 1 1 3 0 2 1 2 2-1 2 2 1 2-9 7-8 7-3 1-3 1-1-3-1 0-1-1-1-2-9-8-9-8-10-8z`,EC:`M291 244l0 3-1 2-3 4-3 2-2 3 0 2-2 2-1-2-1 0-1 0 0-1 1-1-1-2 2-3-1-1-1 1-2-1 1-1-1-4 1 0 1-3 1-2 0-2 1 0 2-2 3 2 1 2 2 0 1 0 1 1 2 1z`,EE:`M559 44l-1 2 1 2 0 1-2 0-2-1-1-1-2 1 0-2-1 1-1-1-1-2 3 0 2-1 3 1 2 0z`,EG:`M599 162l-11 0-10 0-11 0-1-13-1-13-1-2 1-2-1-2 1-2 3 0 3 1 3 1 1 1 2-1 1-1 2-1 2 1 1 2 0-2 2 1 2 0 2 0 2 5 0 1-1 1 0 3 0 1-1 1-1-1-1-2-3-5 0 1 2 3 2 4 2 5 1 2 1 2 3 4 0 1 0 2 3 3 1 1z`,EH:`M477 142l0 1 0 5-9 0 0 9-2 0-1 2 0 6-10-1-1 2 0-2 6 0 1-1 1-2 1-5 4-4 1-4 1-1 1-2 2-1 1 1 1 0 1-1 2 0 0-2z`,ER:`M600 190l-1-2 1-5 0-3 1-1 2 0 1-2 2 4 1 3 2 2 4 3 1 3 2 2 1 1 1 1-1 1-1 0-1-2-1-2-1-1-1-1-3-2-1 0-1 0-2 0-1-1-1 2-3 0z`,ES:`M481 110l0-1 1-2 0-1 0-3-1-2 2 0 0-1 0-1 0-2 1-1 0-2-2 0 0 1-2 0 0-2-1 0-1 1 0-2-1-1 4-3 3 1 3 0 3 0 2 0 3 0 1 2 5 1 1-1 2 2 3-1 0 2-2 2-3 1 0 1-2 2-1 2 1 2-1 2-1 2-2 1-1 2-4 0-2 0-2 1-1 2-1-1-1-1 0-2-3 0z`,ET:`M632 213l-7 12-4 0-2 2-2 1-1 1-2 0-1-2-2 2-1 2-2-1-1 0-1 0-4-3-2 0-1-2 0-2-1 0-2-4-1-1-1-2-1-1-2-1 1-2 1 0 1-1 0-3 0-4 2-1 0-2 1-3 1-2 1-3 1-3 3 0 1-2 1 1 2 0 1 0 1 0 3 2 1 1 1 1 1 2 1 2-1 1 0 2 0 1 0 1 1 0 1 0 1 1-1 1 1 2 1 2 2 1 9 4 2 0z`,FI:`M554 22l1 1 3 2-1 1 3 3-1 2 2 1 0 2 3 1 0 2-2 1-4 3-3 0-4 1-3 1-2-2-2 0 0-3-1-2 1-1 1-2 4-2 1-1 0-1-3-1-1 0-1-4-3-1-3-1 1-1 3 2 2-1 2 1 2-1 0-1 3-1 3 1-1 1z`,FJ:`M991 303l-1 2-2 1-2 1 0-2 2-1 1 0 2-1zM984 308l1 0 0 1-1 2-2 0-1 0 0-2 2-1 1 0zM10 303l0 2-1-2 1 0z`,FK:`M362 423l2-2 2 1 1-2 2 2 0 1-3 1-1-1-1 1-2-1z`,FR:`M357 228l-2 3-1 3-1 1-1 1-1-1 0-1-1 1-1 0 0-2 1-2 0-1-1-2 0-3 1-3 1 0 2 1 3 3 1 2zM514 71l1 1 4 0-1 2-1 2 0 1-1 0-2 2 0 2 1-1 1 1 0 1 1 1-1 1 1 3 1 0 0 2-2 1-5-1-4 1 0 2-3 1-2-2-1 1-5-1-1-2 2-2 0-6-2-3-2-1-3-2-1-2 3 0 4 0 0-3 2 1 5-2 1-2 2-1 0 1 1 0 1 1 2 2 1-1 2 2 1 0zM521 92l2-1 0 2 0 3-2-1 0-2 0-1z`,GA:`M531 235l2 0 1 0 2 0 0 2 1 1 2 0 1 1-2 4 2 2 0 3 0 3-1 2-3-1-1-1 0 1-2 1-1 1 1 2-2 2-3-4-2-3-2-3 1-2 0-1 1-3 0-2 1-1 4 0 0-4z`,GB:`M486 58l-1 0-2 0 1-2-1-1 2 0 3 2-2 1zM493 60l0-2-1-2-3 0 0-1 1-1-1-1-1 1 0-2-1-1 1-3 1-2 2 0 3 0-3 3 2-1 3 0-1 2-2 3 2 0 1 0 2 3 1 1 1 3 1 1 3 1 0 1-2 1 1 2-2 1-3 0-4 1-1-1-1 2-2-1-2 2-1-1 3-3 2-1-3 0-1-1 3-1-2-2 1-2 3 1z`,GE:`M596 89l3 0 3 1 4 2 2 0 2 0 1 2 2 0-1 1 2 1 0 1-2 0-2-1-3 1-3-2-3 0 0-1-1-2-1-1-2-1-1-1z`,GH:`M500 202l0 1 1 2 0 3 0 3 1 1-1 4 1 2 0 2 1 1-4 2-2 2-2 1-3-1 0-2-1-3 1-4 1-3-1-6 0-2 0-3 5 0 1 1 1-1 1 0z`,GL:`M421 3l6 0 6 0 2-1 6 0 14 0 10 1-3 0-7 0-9 0 1 0 6 0 5 1 3-1 2 1-2 0 4 0 8 0 5 0 1 0-7 1-1 0-5 0 4 0-3 1-1 1 0 1 2 1-3 0-3 0 3 1 0 1-2 1 2 1-4 0 2 1-1 0-2 0-3 0 2 2-4 0-1 0 3 1 2 1 0 1-3 0-1 0-2-1 0 1-3 1 5 0 3 0-6 2-5 1-6 1-2 0-2 1-4 2-5 1-1 1-3 0-3 1-2 1-1 1-1 2-4 2 0 2-2 2-2 2-3 0-2-2-4 0-2-1 0-2-2-3 0-2 0-2-1-2 1-1-1-1 3-3 4 0 1-1 1-2-2 1-2 0-2 1-2-1 1-1 1-1 2 0 4 0-3-1-1-1-2 1-1-1 3-1-1-1-1-1-1-2-2 0 1-1-5-1-3 0-5 0-4 0-2 0-2-1 4 0 4-1-7 0-4 0 1-1 7 0 7-1 1 0-5-1 2 0 7-1 2 0 4-1 5 0 6 0 1 1 5-1 4 0 2 0 4 1-4-1z`,GM:`M454 193l3 0 1-1 1 0 1 1 1 0 1-1 0 1-1 1-1 0-2-1-1 1-1 0-2 0 0-1z`,GN:`M462 196l2 0 2 1 2 0 1 1 1 0 1 1 1 0 1-1 1 0 0-1 1 0 1 1 0 1 1 2-1 1 0 1 1 0 0 1 1 1-1 1 0 1 1 1 0 3-1 1 0 1 0 2-1 0-1 1-1-1 0-1-1-3-1 1-1 0 0-1 0-1 0-1-1-2-1-1-2 0-1 1-1 0 0 1-2 2-1-2-1-2-1 0 0-1 0-1-1-1-1-1 2-2 1 0 1 0 0-1 0-1 0-2z`,GQ:`M527 235l4 0 0 4-4 0-1 1 0-1 1-4z`,GR:`M567 116l-1 1-3 0 0-1-3 0 0-2 1 2 2-1 2 1 2 0zM556 96l2 0 2-1 2 1 2 0 0-2 1 1-1 2 0 1-2 0-1-1-3 1 2 2-1 0-2 0-1-1 0 2 2 2-1 1 1 1 2 1 0 2-2-1 1 2-2 0 1 3-1 0-3-1-1-3 0-2-2-2-1-2 0-1 1-1 0-1 1-1-1 0 2-1 1 0 1 0 1-1z`,GT:`M248 189l0-1 0-1 2-3 3 0 1-2-1 0 0-1-1 0-1-2 2 0 0-2 3 0 2 0 0 3-1 4 1 1 1-1 0 1-1 1-2 1 0 1 0 1 0 1-1 0-1 1-1 1-1 0-2 0-1-1-1-2z`,GW:`M454 197l2 0 1 0 0-1 5 0 0 2 0 1 0 1-1 0-1 0-2 2-1-1-1-1-1-1-1-1 0-1z`,GY:`M343 236l-1 0-1 0-1 1-1 1-1 0-1 1-1-1-2-1 0-2-1-2 1-3 1-1-1-2-1-1 1-1-1-1-1 0-2-3 1-1 0-2 1 0 1-1-1-1 1-2 2-2 2 1 1 3 0 2 1 0 2 2 1 1-1 3-2 1 1 1-1 2 1 3 1 0 0 2 2 3z`,HN:`M273 188l-1 0-1 0-1 0-1 1-1-1-1 1 0 1-1 1-1 0 0 1-1-1 0 1-1 0-1 0 0 2-1 1-1 0 0-1-1 0 1-2-1 0 0-1-1 1 0-1-1 0-1-1 0-1 0-1 0-1 2-1 1-1 1 0 0-1 1 0 0 1 1-1 1 1 1 0 1-1 1 0 1 0 1 0 1 0 1 0 1 1 0 1 1 1 1 1z`,HR:`M539 80l1 0 2 2 2 0 1 0 0 1 1 1-1 1-1-1-1 0-3 0-1 0-1 0 0 1 1 1 0 1 2 2 1 0 1 2 2 1-3-1-1-1-2-1-3-2 1 0-1-2-1-1-1 0-1 1-1-1 0-1 1 0 1 0 1-1 0 1 1 0 0-1 1 0 0-1 2-1z`,HT:`M306 170l0 2 0 2-1 0 1 1 0 1-2 0-2 0-1 0-2 0-1-1 1-1 2 1 2 0 1-1-1-1 0-2-1 0 1-1 1 0 2 0z`,HU:`M551 74l2 1 0 1-1 0-1 2-1 2-2 1-2 0-1 1-1 0-2 0-2-2-1 0-1-1 1-2-1-1 1 0 0-1 2 1 1 0 2 0 0-1 1 0 1 0 1-1 1-1 3 1z`,ID:`M891 253l0 12-2 12-2-3-2 0-1 1-4 0 2-3 2-1-1-4-1-3-5-4-2 0-4-3-1 1-1 1-1-2 0-1-2-2 3-1 2 0 0-1-4 0-1-3-2 0-1-2 3-1 2-1 4 1 1 2 0 6 3 2 2-4 4-2 2 0 3 1 2 1 2 1zM845 277l0 1-2 3-2 1-1 0 1-2 1-2 3-1zM872 269l0-3 0-1 1-1 0 1 0 2-1 2zM827 228l-1 3 2 4-1 2 4 3-4 0-1 3 0 3-2 3 0 4-2 5 0-1-3 2-1-3-2 0-2-1-3 1-1-1-2 0-2-1 0-5-2-1-1-3 0-3 0-4 2-2 0 2 2 2 2 0 2 0 1-2 1 0 3 1 2-1 2-5 1-1 0-5 4 0 2 1zM859 254l3 1 1 3-2-2-3 0-1 0-2 0 1-2 3 0zM852 258l-2-1 0-2 3 0 0 1-1 2zM855 235l0 2 2 1 0 1 0 3-1 0-1 2 2 2-1 1-1-3-1-4 0-3 1-2zM841 240l4 0 2-3 1 1-2 4-2 0-3 0-5 0-2 0-1 3 3 4 1-2 6-1-1 1-1 0-1 2-3 1 3 5-1 2 3 4-1 2-1 1-1-1 1-3-2 2-1-2 0-1-1-2 0-4-2 1 0 5 0 5-2 1-1-2 1-3 0-4-2 0 0-2 1-3 0-3 2-5 0-2 3-3 2 2 3 0zM832 282l-4-3 3-1 1 1 1 2 0 1-1 0zM835 275l2 0 3-1-1 2-4 1-4-1 0-1 3-1 1 1zM827 275l2-1 0 2-3 1-2 0-2 0 2-2 1 0 1-1 1 1zM800 267l1 2 5 0 1-1 5 1 1 3 4 0 3 3-4 1-2-1-3 0-3-1-2 0-3-2-2 0-2 0-4-1-1-2-2 0 2-4 3 1 2 1 1 0zM790 247l0 3 1 2 2 0 2 3-1 4-1 6-3 0-2-3-3-3-2-2-1-3-2-3-2-5-2-3-1-4-1-2-3-3-1-3-2-2-4-4 0-2 2 0 4 1 3 3 2 3 2 1 2 5 3 0 3 2 1 3 2 2-1 3 2 1 1 0z`,IE:`M486 58l1 2-2 3-4 2-4-1 2-3 0-3 3-2 1-1 1 1-1 2 2 0 1 0z`,IL:`M592 125l-1 1-1-1 0 2 1 1-1 0 0 1 1 0 1 1-1 6 0-1-2-5 0-2 0-1 1-3 0-1 1 0 1 0 0 1 0 1z`,IN:`M755 140l0 1 0 1 1 2-2-1-3 3 0 2 0 3 0 2 0 2-3 0 1 3 0 2 1 1-2 1-2-6-1 0 0 3-2-2 0-2 2-1 0-3-2 0-2 0-3-1-1-2-1 0-2-2-1 3 3 2-2 1 0 1 2 1 0 2 2 3 1 3 0 1-2 0-3 1 0 3-1 2-3 2-2 4-2 3-2 2 0 2-1 1-3 1-1 0 0 3 1 5 0 3-1 3 1 7-1 0-1 3 1 1-3 1-1 3-1 1-2-4-2-5-2-4-1-2-1-3-2-5 0-2-4-5-2-7-1-5-1-4-1-4-3 3-2-1-5-4 1-2-1-1-3-3 1-2 6 0-1-3-2-2-1-3-2-1 2-4 3 0 2-4 1-3 1-4 0-2 1-2-2-2-2-2-1-3 0-1 5 0 2 0 2-3 4 4 0 3 2 2 0 1-2 0 2 4 3 2 5 2-2 2 0 3 3 1 3 2 4 2 4 0 2 2 2 1 3 0 2 0 0-1-1-2 0-2 1-1 1 3 1 1 2 1 2 0 2 0 2 0 0-2-1-1 2-1 1-2 3-3 2 1 1-1 2 2 0 1 3 1z`,IQ:`M601 126l-2-4 5-3 1-4-1-3 1-1 1-2 1 0 3 0 1 1 1 0 3 3 1 1 1 2-1 2 0 2 2 3 4 2 1 3 0 2 1 0 0 2 2 2-1 0-2 0-2 3-4 0-9-7-4-3-3-1z`,IR:`M626 134l-2-2 0-2-1 0 0-2-1-3-4-2-2-3 0-2 1-2-1-2-1-1-3-3-2-3 1-1-2-4 1-1 1 1 2 2 1 0 1 0 3-2 1 0 1 1-1 1 2 2 1 0 1 2 2 1 3 2 3 0 4-1 0-1 2 0 1-2 2 0 1-1 2 1 3 1 2 1 4 3 2 0 1 3 0 4-1 2 2 1-1 2 2 2 0 2 2 1 1 2-2 3 2 2 1 2 3 2 1 3 1 1 1 1-4 2 0 4-5-1-3-1-3 0-2-5-1 0-2 0-3 2-3-1-3-3-3-1-2-3-3-5-1 1-2-1-1 1z`,IS:`M472 27l-1 2 2 1-3 2-6 2-2 0-2 0-6-1 2-1-4-1 4-1-4-1 2-1 3 0 2 1 4-1 2 0 3-1 4 0z`,IT:`M525 79l1 0 2-1 1 1 4 1-1 1 1 2-2-1-2 1 1 2-1 1 1 1 3 2 1 2 3 3 2 0 0 1 2 1 2 1 2 2 0 1 0 1-1-2-3 0 0 2 1 1 0 2-1 0-1 3-1 0 0-1 0-2 1 0-1-2-1-2-1 0-1-2-2 0-1-2-2 0-2-1-2-2-2-2-1-3-1-1-2-1-1 1-1 1-1 1 0-2-1 0-1-3 1-1-1-1 0-1 1 1 1 0 2-1 1 0 1-1 1 0 1 0 1-1zM537 106l2 0-1 3 0 1 0 1-2-1-1 0-4-2 0-2 3 1 3-1zM521 97l2-1 1 3 0 4-1 0-1 1-1-1 0-4-1-2 1 0z`,JM:`M290 175l2 0 1 1 0 1-2 0-1 1-1-1-1-1 0-1 1 0 1 0z`,JO:`M591 126l1-1 3 1 4-4 2 4-1 1-5 2 4 3-1 1-1 1-1 0-1 2-1 1-3-1 1-6-1-1 1-1-1-2z`,JP:`M851 103l0 3 2 4 1 2 1 2 0 2-2 2-4 0-2 4-2-1-2-3-4 1-2 1-3 1 4 2 1 6-1 1-2-1-1-3-2-1-2-2 2-2 0-2 2-1 0-3 5-1 3 1-1-6 3 2 1-4 1-1-1-4-3-4 0-2 2 0 4 4 2 3zM847 88l0-2 4 4-3 1 0 3-5-2 1 3-3 0-2-3-1-2 2-1-2-4-1-2 5 3 3 1 2 1zM838 122l1-2 1 0 1-1 2 1 1 1-1 2-1-1-1 1 0 2-2-1-1-2z`,KE:`M609 261l-4-4 0-2-10-8-1 0 0-4 1-2 1-2 1-3-1-4 0-2-2-3 2-2 2-2 1 0 0 2 1 2 2 0 4 3 1 0 1 0 2 1 1-2 2-2 1 2 2 0-2 4 0 14 1 3-1 1-1 2-1 0-1 3-1 1 0 3-1 1z`,KG:`M672 93l0-1 2-1 4 1 0-2 1 0 4 1 5 0 3 0 2 1 2 1-4 2 0 1-3 1 0 2-3-1-1 1-2 2 0 1-4 1-4-1-2 0-1-2 3 0 1-1 2 1 2-3-4-1-1 1-2-2 1-2-1 0z`,KH:`M782 198l-1-5 1-3 3 0 3 0 2 2 1-3 3 1 1 3 0 4-4 3 2 2-3 1-2 1-3 0-1-2-2-4z`,KP:`M817 93l-1 0 0 1 0 1 2 2-1 1 0 1-1 1-1 0-1 1 1 1 0 1 1 0 3 2 0 1-1 0-2 0 0 1-1 0 0 1-2-1 0 1-1-1-1-1-1-1 1 0-1-1 0-2-1 0-1-1-2-1 1-2 1-1 1-3 2 1 2 1-2-2 3-2 0-1 3 2z`,KR:`M815 108l0-1 1 0 0-1 2 0 1 0 0-1 4 4 2 2 2 4 0 1-2 1-1 2-2 0-2-2 0-3-3-3 1-1-3-2z`,KW:`M625 134l0 1 0 1 2 3-2 0-1-2-3 0 2-3 2 0z`,KZ:`M701 72l0 2-2 0 1 3 0 1-5-1 1 6-1 0-5 2 5 5-1 1 1 2-2-1-2-1-3 0-5 0-4-1-1 0 0 2-4-1-2 1 0 1-1 1-2 2-1 2-1 0-1-1-3 0-1-3-2 0 0-3-4-3-4 1-3 0-3-3-2-1-5-2-6 1 4 12-2 0-2-2-2-1-2 0-1 2-1-1 1-2-1-1-3-1-2-3-2 0 0-1 3 0-1-2 2-1 2 1 0-3-1-2-3 0-2-1-2 2-2 0-2 0 0-2-2-2-2 0-2-2 1-2-1-1 1-3 2 2 0-2 4-3 3-1 6 2 3 2 2-2 4 0 3 2 0-1 4 0 0-1-5-2 2-2-1 0 2-1-3-2 1-1 8-1 1-1 5-1 1-1 4 1 2 3 2-1 4 1 0 1 2 0 5-2-1 1 4 2 9 7 1-2 4 2 2-1 2 1 2 2 2 0 1 1 3 0 2 2z`,LA:`M794 190l-3-1-1 3-2-2 1-1-1-4-3-3-1-3-2-3-2-1-1 2-1 0-1-1-3 2 0-3 0-4-2 0-1-2-1-1 0-2 2-2 0 1 2 0-2-4 1-1 3 3 2 4 3 0 2 3-2 1 0 1 4 2 3 4 2 4 3 2 1 3 0 3z`,LB:`M592 123l-1 0-1 0 0-3 1-2 2 0 0 1-1 2 0 2z`,LK:`M726 215l0 4-1 1-2 1-2-3-1-5 1-6 2 2 2 2 1 4z`,LR:`M477 215l0 1 0 2-1 1 1 1 1 0 1 2 0 1 0 1 0 3-1 0-3-2-2-3-3-2-2-2 1-1 0-1 1-2 2-2 1-1 1 3 0 1 1 1 1-1 1 0z`,LS:`M576 349l0 1-1 2 0 1-2 1 0 1-1 0-2-3 2-2 1-1 2-1 1 1z`,LT:`M558 54l0 1-1 1-1 1-2 1-2 0-1-1-1 0 0-1-1-1-2 0-1-2 2-1 4 0 2 0 1 0 3 2z`,LU:`M514 69l0 1 0 1-1 0 0-2 1 0z`,LV:`M559 49l1 0 0 2 1 1-2 1-1 1-3-2-1 0-2 0-4 0-2 1 0-2 0-2 2-1 2 2 2 0 0-2 2-1 1 1 2 1 2 0z`,LY:`M567 162l0 7-3 0 0 2-11-7-11-7-2 2-2 1-2-2-4-1-1-2-2-2-2 1-1-2 0-2-1-3 1-1-1-2 1-2-1-2 1-3 0-1-1-3 1-1 0-1 0-2 1-1 1-1 1-1 0-3 3 1 2 0 2 1 3 1 2 3 2 1 4 1 3 2 1-1 1-2-1-2 1-2 1-1 2-1 4 1 1 1 1 0 1 1 2 0 1 1-1 2 1 2-1 2 1 2 1 13 1 13z`,MA:`M495 116l0 2 1 2 0 4 1 1 0 1-4 1-1 1-1 0-1 3-3 1-1 2-2 1-2 0-5 3 0 4 0 2-2 0-1 1-1 0-1-1-2 1-1 2-1 1-1 4-4 4-1 5-1 2-1 1-6 0 0-1 2-1 1-2-1-2 1-2 2-2 1-1 1-2 0-2 1-2 2-1 2-4 1-1 2-1 3-2 1-1 2-3 0-4 1-3 0-2 2-2 2-2 2-1 2-4 1-2 2 0 1 2 3-1 2 1 2 0z`,MD:`M562 75l0-1 2 0 2 1 1 0 1 1 0 1 1 0 0 2 1 0 0 1 1 0-1 0-1 0-1 0 0 1 0 1 0 1-1 0-1-1 0-2 0-1-2-2-1-1 0-1-1 0z`,ME:`M549 92l-1 0-1 1 0 1-1-1-1-1 0-2 1-1 1 1 1 0 1 1 0 1z`,MG:`M636 290l1 1 0 3 0 4 1 2-1 2 0 1-1-2 0 1 0 2-1 2 0 1-1 3-1 4-2 5-3 7-1 5-2 4-2 1-3 1-1 0-2-2-1-2 1-3-1-3 0-2 1-3 1 0 0-2 2-2 0-3 0-1-1-3 0-3 2-2 0-2 1-1 2 0 1-1 1 0 2-2 2-2 1-2 0-2 1 1 2-3 0-2 1-2 1 2z`,MK:`M554 93l2 1 0 2-1 1-1 0-1 0-2 1-1-1 0-2 0-1 2-1 1 0 1 0z`,ML:`M468 197l1-1-1-2-1-1 0-2 0-2 1-1 0-2 1 0 2 1 1-1 2 1 0-1 11 0 1-3-1 0-1-16-1-15 4-1 9 8 9 8 1 2 1 1 1 0 1 3 3-1 0 9-2 2 0 2-2 1-4 0-1 2-2 0-2 0 0-1-2 1-2 1-1 1-2 2-1 1-1 1-1-1-1 1 0 3-2 3 0 1-1 2 0 2-1 0-1 1 0-2-1 1 0-1-1 2-2-1-1 0-1-1 0-1-1 0 0-1 1-1-1-2 0-1-1-1-1 0 0 1-1 0-1 1-1 0-1-1-1 0-1-1z`,MM:`M770 168l-2 1-1 1-2 0 0 4-1 1 2 3 2 3 2 2-1 3-1 1 1 2 2 3 1 2 0 2 2 3-1 3-1 4-1-2 1-3-1-2-1-5-1-1-2-5-1-4-2-4-1 2-3 3-1 0-2-1 0-5-1-3-3-4 0-2-2 0-2-3-1-3 1 1-1-3 2-1-1-1 0-2-1-3 3 0 0-2 0-2 0-3 0-2 3-3 2 1-1-2 0-1 0-1 1 0 2 2 1 1 1 2 1 3-2 3 1 4 3 0 1 3 2 1 0 3 2 1 2 1 1-1 1 1-2 2 0 2z`,MN:`M702 71l2 0 3-3 3-1 2 1 3 0 2 1 3 0 3 1 1-2-2-2 1-2 3 1 2 0 3 1 2 2 4 1 2 0 3-1 2 1 4 1 2 2 2-1 3 1 2-1 3 0 2-2 1 0 2 1 3 0 0 2 1 3 1 1 1 0 3 0 1-1 2 1 4 2 0 1-2 0-3 0-1 1 0 2-3 1-2 2-2-1-2 0 0 2 2 1 1 1-1 1-1 2-2 1-4 0-3 1-2 2-2-1-3 0-4-2-3 0-3 0-5-1-3 1-2-2-3-3-2-1-3-1-4-1-3 0-1-2-1-3-3-3-4-1-3-1-2-3z`,MR:`M454 166l1-2 10 1 0-6 1-2 2 0 0-9 9 0 0-5 10 8-4 1 1 15 1 16 1 0-1 3-11 0 0 1-2-1-1 1-2-1-1 0 0 2-1 1-2-3-2-2-1-1-2-1-1 0-2 1-1-1-1 1 0-2 1-1 0-4 0-3 0-2 0-2-1-2-1-1z`,MW:`M590 278l3 1 1 1 1 1 0 5-1 3 1 5 1 0 1 1 1 3 0 4-2 1-1 3-1-3 0-2 0-2 0-1-1-1-1 0-1-2-1-1 1-3 0-1 0-3 1-3 0-1 0-4-2-1z`,MX:`M199 125l3 0 4-1-1 1 3 2 5 2 5 0 3 0 0-1 5 0 0 1 1 1 1 2 0 1 0 2 1 1 2 1 2-2 3 0 1 1 0 2 1 2 1 2-1 3 1 2 1 1 2 0 1 0-2 3-1 3-1 5-1 2 0 1 0 2 0 3 2 3 0 2 0 2 3 1 1 1 3-1 2 0 2-1 2-1 2-1 1-2 1-3 0-1 2-1 3-1 3 0 1 0 1 1-1 1-2 2-1 3 1 0-1 2-1 3-1-1-1 0-1 2-1 0-2 0-3 0 0 2-2 0 1 2 1 0 0 1 1 0-1 2-3 0-2 3 0 1 0 1-3-4-1-1-2-1-2 0-2 2-1 0-2-1-2-1-2-1-2-1-3-1-2-2-1-1-1 0-3-1 0-2-3-2-1-2 0-2 1-1 0-1 1-1 0-1 0-1 0-2 0-2-2-3-2-3 0-2-2-2 0-1 1-2-1-1-1-1 0-3-1 0-1-2-1-2 0-1 0-2 0-3 1-1-2-2-1 1-1-1-1 1 0 2-1 2 1 2 1 2 0 1 0 1 0 3 1 0 0 2 1 1 0 4 0 1 0 2 0 2 1 0 1 2 0 1 0 1-2 1 0-2-1-2-2-2-1-1 1-2 0-2-1-1-1-2 0 1-1-1-1-1-1-2 0-1 1 1 2-2 0-1-1-3-1-1 0-2 0-2 0-3 0-3z`,MY:`M777 219l1 0 2 1 0 2 2 0 1-2 1 0 1 3 2 2 0 3 0 1 0 2 0 2 1 1 2 3-1 2-1 0-3-3-4-3 0-2-2-2 0-3-1-3 0-2-1-2zM827 228l-2-1-4 0 0 5-1 1-2 5-2 1-3-1-1 0-1 2-2 0-2 0-2-2 0-2 2 1 2-1 0-3 1 0 4-1 2-3 1-3 1 2 1-1 1 0 0-2 0-2 2-3 1-3 1 0 2 2 0 2 2 1 2 1 0 2-2 0 1 2-2 1z`,MZ:`M595 286l2 0 3 1 1 0 2-1 1-1 2 0 3-1 2-2 0 2 0 3 0 3 0 6 0 2 0 3-2 2-2 2-2 2-3 2-4 4-1 0-2 3-1 1-1 2 2 3 0 2 0 1 0 3-1 2 1 1-1 1-1 1-2 2-4 2-1 1 0 1 1 1-1 2-1-1-1-1 0-2 0-1 1-4-1-2 0-5 3-5 1-2 0-3 0-1 0-2 1-3 0-5-1-1-1 0-1-1-1-1-2 0 0-1 0-3 8-3 1 2 1 0 1 1 0 1 0 2 0 2 1 3 1-3 2-1 0-4-1-3-1-1-1 0-1-5 1-3z`,NA:`M553 335l-1 13-2 1-2 1-1-1-2 0 0-2-1-1-1 2-2-3-1-2 0-4-1-2-1-6 0-4 0-2-1-1-1-3-1-5-1-2-2-4 0-2 1-1 2-1 2 0 1 2 1 0 11 0 1 1 7 1 5-2 2 0 2 0 1 1-1 1-1 0-2 1-1-1-4 1-2 0-1 13-3 0 0 11z`,NC:`M946 321l1 3 1 1-2 1-1-1-1-2-1-1-1-3 0-1 1 0 2 1 0 1 1 1z`,NE:`M540 159l1 6 1 1 0 1 1 1-1 1 0 8 0 5-4 3-1 5 1 1 0 3 2 0 0 1-1 1 0 1-1 0-1-4-1 0-2 2-2-1-2-1-1 1-1 0-2 1-1 1-4-2-1 1-1 0-1-2-3-1-3 0-1 1 0 2-1 2 0 3-2-2-1 0-1 1 0-3-3-1 0-1-2-3 0-1 0-2 2 0 1-2 4 0 2-1 0-2 2-2 0-9 3-1 8-7 9-7 4 1 2 2 2-1z`,NG:`M507 220l1-6 0-2 0-3 1-1 1-2 0-1 0-2 0-2 0-1 0-3 1-2 0-2 1-1 3 0 3 1 1 2 1 0 1-1 4 2 1-1 2-1 1 0 1-1 2 1 2 1 2-2 1 0 1 4 1 0 1 1 0 1 0 1-3 3 0 2-1 2 0 1-1 3-1 1-1 2 0 2 0 1-2 2-2-2-1 0-2 2-2 4 0 3-3 1-1 0-1 0-3 0-1-2-1-3-2-2-2 0-3 0z`,NI:`M270 203l-1 0-1 0 0-1-1 0-1 1-1-1-1 0-1-1-1-2 0-1-1-1-2-2 1 0 1 0 1-1 0-2 1 0 1 0 0-1 1 1 0-1 1 0 1-1 0-1 1-1 1 1 1-1 1 0 1 0 1 0-1 1 0 1 0 1-1 2 0 1 0 1 0 2-1 0 0 2 0 1-1 1 0 1 1 1z`,NL:`M515 60l1 0-1 3 0 1-1 0 0 3-1-1-2-1-2 1-1 0 1-1 2-4 3-2 1 1z`,NO:`M526 6l1-1 2 0 2 1 6 1-4 0-1 1-1 0-1 1-2 0-4-1 2 0-3 0-4-1-1-1 5-1 1 1 2 0zM559 21l-3 1-2 0 1-1-3-1-3 1 0 1-2 1-2-1-2 1-3-2-1 1-1 0 0 1-4 0 0 1-2 0-1 2-1 2-3 3 1 1-1 1-2 0-1 2 1 3 1 1 0 3-2 2-1 1-1-1-4 3-3 0-3-1-1-3-1-5 2-2 5-2 4-2 3-3 4-4 3-2 5-2 4-1 3 0 3-2 3 0 3 0 7 1-3 1 3 1zM547 5l-2 1-5 0-5 0-1 0-2 0-2-1 5 0 2 0 2 0 4 0 4 0zM543 8l-4 0-3 0 1 0-1-1 4 0 1 1 2 0z`,NP:`M731 141l0 2 1 2 0 1-2 0-3 0-2-1-2-2-4 0-4-2-3-2-3-1 0-3 2-2 0-1 3 1 3 3 2 0 1 2 2 1 3 1 3 1 3 0z`,NZ:`M935 387l-2 1-3 3-3 1 0-1-1 0 4-3 1-2-2-1 1-1 3-1 2-3 2-2 1-2 0-1 0-1 0-3 1-3 1 0 0 2 1 1-1 3-1 3 2-2 0 1-1 2 1 2 1 0 3-1 1 0-3 3-3 2-1 0-2 1-1 1-1 1zM908 398l4-2 2-2 4-2 1-1 2-1 3-2-1 1 0 2 3-2-1 2-1 1-2 2-4 2-2 1-1 2-1 0-4 1-2 2-4 3-4 1-2 1-2 0 0-1-3 0 1-1 3-2 6-3 2-1 3-1z`,OM:`M648 160l0-2 0-1 0-2 1-1-1 0 0-2 2 0 1 2 2 1 2 1 2 0 1 3 1 1 1 0 0 1-1 2 0 1-1 1 0 3-2 0 0 2 0 3-1 0-2 1 0 2 0 1-2 0-1 1 0 1-1 1-2 0-1 1-1 1-2-3-2-6 7-4 1-7-1-2zM649 149l-1-1 1-2 0 1 0 1 0 1z`,PA:`M286 211l0 1 0 2 0 1-1 0-1 1 0-1-1-2 1-1-1 0 0-1-1-1-2 0 0 1-2 1 0 1 1 2-1 1-1 0-1 0 0-2-1 1-1-2-1 0-1-1-1 0 0 1 0-1 0-1 0-1 1 0-1-1 0-1 1 0 1 1 0 1 1 0 1 1 1-1 2-1 1 0 1-2 2 1 1 0 2 1 0 1 1 1z`,PE:`M306 259l-2 0-1 1-2 1-3 2 0 2-1 1 1 2-2 1 0 2-1 0 2 4 1 2 0 2 2 0 1 2 2 0 2-2 1 5 1 1 2-1 2 6 0 1 0 3 0 3 0 2 0 1 0 2 1 3-1 4 0 1-2 1-3-2 0-1-6-4-5-4-3-2-1-3 0-1-3-5-3-7-3-7-2-2-1-3-2-2-2-2 1-1-2-4 1-3 2-2 1 2-1 1 0 1 1 0 1 0 1 2 2-2 0-2 2-3 3-2 3-4 1-2 0-3 2 1 1 2 1 1 2 4 2 0 2-1 1 1 1 0 3 2-2 3 1 0 1 2z`,PG:`M891 253l5 3 5 2 2 2 1 2 1 2 4 2 1 2-3 0 0 3 2 2 2 4 1 0 0 2 2 1-1 0 3 2-1 1-2 0 0-1-3 0-2-1-2-2-1-2-1-4-4-1-2 1-2 1 0 3-2 1-2 0-3-1 2-12 0-12zM924 257l1 1 0 2-1 1-1-2 0-1-1-2-2-1-2-1 1-1 2 1 0 1 2 0 1 2zM919 265l-1 1-2 1-1 0-3-1-1-1 0-1 3 0 1 0 1-2 0 2 2 0 1-2 2-1-1-2 2 0 1 0-1 3-1 2-1 0-1 1zM929 263l1 1 1 3 1 1 0 1-1 0-1-1-1-3-1-2 1-1 0 1z`,PH:`M832 196l-2-3 2 0 1 2 0 3-1-2zM838 206l1-1 0-2 1 0 0 2 2-4 0 4-1 1-1 3 0 1-2-3 0-1zM849 212l1 2 0 2 0 4-2-4-1 2 1 3 0 1-4-2-1-2 1-2-2-2-1 2-1-1-2 3-1-2 1-3 2-1 1-1 1 2 2-1 1-2 1 0 0-3 2 2 1 2 0 1zM827 209l-3 3 1-2 2-3 1-2 1-4 1 3-2 2-1 3zM832 176l0 1 1 3 0 3-1 1 0 3 1 3 2 0 1 0 4 2 0 2 1 1 0 2-3-2-1-2-1 1-2-2-2 1-2-1 0-2 1-1-1-1 0 2-2-2-1-2 0-3 1 1-1-6 0-3 2 0 2 1 0-1 1 1zM836 201l-1-2 2 1 2 0 0 2-1 1-2 1 0-1 0-2zM845 198l1 4-2-1 0 1 1 3-1 1 0-3-1 0-1-3 2 1-1-2-2-2 3 0 1 1z`,PK:`M697 115l-2 3-2 0-5 0 0 1 1 3 2 2 2 2-1 2 0 2-1 4-1 3-2 4-3 0-2 4 2 1 1 3 2 2 1 3-6 0-1 2-2-1-2-2-2-3-5 1-4 0-4 0 0-4 4-2-1-1-1-1-1-3-3-2-1-2-2-2 5 2 3 0 1 0 1-1 1 1 3-2 0-3 1-2 2 0 0-1 2 0 1 0 1-1-1-2 0-2 2-1-2-2 3 0 0-1 0-1 0-2 0-1-1-2 1-1 2-1 3 0 1-1 1 0 3 1 1 3 5 1z`,PL:`M552 58l0 2 1 1 0 1-1 0 1 2 0 1 2 2 0 1-1 1-2 2 1 1-1 0-2-1-2 0-1 0-1 1-2-1-1 0-1-2-2 0 0-1-1 0-1 1-1-1 0-1-2 0-1-1-1-2 0-1-1-1-1-1 1-1-1-1 2-1 3-1 3-1 2 0 0 1 2 0 3 0 4 0 1 0 1 1z`,PR:`M320 175l2 0 0 1-1 1-2 0-1 0 0-2 2 0z`,PS:`M591 129l-1 0 0-1 1 0-1-1 0-2 1 1 1 2-1 1z`,PT:`M478 94l1-1 1 0 0 2 2 0 0-1 2 0 0 2-1 1 0 2 0 1 0 1-2 0 1 2 0 3 0 1-1 2 0 1-1 1-1-1-1 1 0-3 0-2-1 0-1-2 1-2 1-1 0-1 1-2 0-2-1-1 0-1z`,PY:`M343 318l1 2 1 5 3 1 1-1 2 1 0 1 1 3 0 2 1 0 1-1 1 1 1 2 0 2 0 2 0 3-2 3-2 0-3 0-3-1 1-6 0-1-3-1-4-3-2-1-6-5 1-5-1-2 1-3 5-1 2 0 3 2 0 1z`,QA:`M635 152l-1-2 1-2 0-1 1 2 1 2 0 2-1 0-1-1z`,RO:`M567 83l1 1 1-1 1 1 1 0-2 1 0 3-2 0-2-1-2 1-2 1-3-1-2 0-1 0-1-1 0-1 0-1-1 1-2-1 0-1-1-1-1-1-1-1 2-1 1-2 1-2 1 0 1-1 1 0 2 0 1 1 1 0 1-1 1 0 1 0 1 0 0 1 1 1 2 2 0 1 0 2 1 1z`,RS:`M545 82l1-1 2 0 1 1 1 1 1 1 0 1 2 1 1-1 0 1 0 1 0 1 0 1 1 1 0 1-1 1 1 0-1 1-1 0-1 0 0-1 1 0-1 0 0-1-1 0 0-1-1 0 0 1-1 0-1-1-1 0-1-1 1 0 0-2-1-1 0-1-1 0 1-1-1-1 0-1z`,RU:`M833 18l1 0 2 1-2 0-1-1zM616 80l-1 2-2 0-2 4 3 3 1 2 3 3-1 2 0 1-1-1-2-2-1 0-2 0-1-2-2 0-2 0-4-2-3-1-3 0-3-2-3-1-3-2 2-1 1-2-1-2 3-1-2 0-1-1 1-1 3 0 0-1-1-2 0-2-4-1-1 0-2-2-1 1-3-1 0-1-2-1-1 0-1-1 1-1-2-1-2 0-1 0-1 0-1-1-1-1 0-1 2 0 1 0-1-1-2 0 0-1-1 0-1-2 0-1-1-1-2-1-1 1 0-1-3-1-1-1 0-2-1 0 0-1-1-2 1-2 2-2-3-1 4-3 2-1 0-2-3-1 0-2-2-1 1-2-3-3 1-1-3-2-1-1 2 0 3-1 2-1 3 2 6 0 8 2 2 1 0 2-2 1-3 0-9-1-1 0 4 2 0 1 1 2 3 1 1 0 0-1-1-1 1-1 5 2 2-1-2-2 3-2 2 0 2 1 1-1-2-1 0-2-2-1 6 1 1 1-2 0 1 1 2 1 2 0 0-2 4-1 5-2 2 1-1 1 2 0 1-1 4 0 2-1 3 2 1-2-2-1 6 0 3 1 9 2 0-1-2-1-1 0-2-1 0-1-2-1-1-1 3-1 0-2 1 0 5 0 1 1-1 2 2 0 2 2 1 2 3 1 0 2-1 3 2 0 0-1 2 0 0-1 1-1-2-1 0-2-2 0-2-1 0-2-3-1 2-2-1-1 1 0 1 1 1 2 2 0-2-1 3-1 4 0 4 1-3-1-2-2 3 0 4 0 4-1-2 0 1-1 2 0 3-1 4 0 0-1 5 0 2 0 3 0 3 0 0-1 1 0 3-1 4 0-2 1 4 0 1 1 2-1 5 0 5 1 2 1-1 1-4 1 2 0 3 1 1-1 3 1 3 0 6 0 1 1 8 0-1-1 4 0 3 0 4 1 2 1 0 1 4 1 4 1 0-2 4 1 2-1 5 1 0-1 4 1-4-2 1-1 18 1 4 1 7 2 7-1 4 1 3 0 2 2 3 0 2 0 3 0 5 0 3 0 6 2 1-1-3-1-1-1 7 1 4 0 7 0 4 1 15 9-1 1-3-1 4 2 4 2 2 0 2 1 0 1-4-1-3 2-2 0 0 2-1 1 1 2-6-2-2 2-3-1 0 1-3 0 1 1 1 3 1 1 4 0 4 4-2 0 2 3 2 1-2 1 3 3-3 1 3 3-1 2-3-1-5-5-8-6-3-3 0-2-2-1 3-1-1-3 1-3 1-1-4-4-2 0 2 2-2 3-6-3-4 1 1 4 3 1-4 1-3 0-2-1-3-1-2 1-7 0-6 1-1 5-2 6 4 0 3 2 3 1 0-2 2 1 7 3 2 2 1 3 3 3 3 5 0 4 1 2-1 4 0 4 0 1-2 2-2 0-2-1-2 2 0 1-1-2 1 0-2-4-3-2 1-2 4 1-1-3-2-3 0-1-1-3-3 1-1 1-4 0-3-3-4-2-5-1-3-3-2-1-2-2-4-3-3-1-4 0-3 0-2 0-1 2 2 0 1 2 0 1 0 3 1 1-2 2-3-1-3 0-2-1-1 0-2 2-3 0-2 1-3-1-2 1-2-2-4-1-2-1-3 1-2 0-4-1-2-2-3-1-2 0-3-1-1 2 2 2-1 2-3-1-3 0-2-1-3 0-2-1-3 1-3 3-2 0-1 1-2-2-3 0-1-1-2 0-2-2-2-1-2 1-4-2-1 2-9-7-4-2 1-1-5 2-2 0 0-1-4-1-2 1-2-3-4-1-1 1-5 1-1 1-8 1-1 1 3 2-2 1 1 0-2 2 5 2 0 1-4 0 0 1-3-2-4 0-2 2-3-2-6-2-3 1-4 3 0 2-2-2-1 3 1 1-1 2 2 2 2 0 2 2 0 2 2 0zM660 4l3 0 4 1 5 1 1 1-4 0-5-1-3 0-3-1-2 0 4-1zM677 6l5 1 1 0-10 1 2-2 1 0 1 0zM746 10l5 0 7 1 1 1-7 0-2 0-5-1-1-1 2 0zM765 11l5 1-1 0-3 0-5 0 0-1 4 0zM755 14l2 0 4 0 1 1-3 0-4-1zM577 5l3 0 2 0 1 0 1 0 1-1 3 1-1 0-2 0-1 0-1 0-1 0-2 0-3 0zM550 57l-4 0-3 0 1-1 3-1 2 0 1 1 0 1zM597 14l3-2 3-1 5-1 6 0 3-1 3 0 2 1-1 0-6 1-5 0-5 2-1 1-2 2 1 1 5 2-1 0-7 0-1-1-3 0-1-1 2-1-1-1 3-1-2 0zM817 59l4 3 2 2 4 3 7 5-4-1 2 5 5 3 2 2-3-2 0 2-3-2-2-3-3-3-1-2-4-4-4-3-3-4 0-1-2-2 1 0 2 2zM160 26l-2 1 0 1 2-2 5 0 1 2-4 1-3 1-4 2-2 0-1 0-1-1-1 0 1-1-1-1-3 1 0-1 2-1-3 1-1 1-3 1 15-9 2 2 1 2zM166 19l-2 0 2-1 0-1 2 0 2 1-1 0-3 1zM579 81l1 0 1 0 1 0 1 1 1 1 3 0-1 1-2 1-3 1-1 0 0-2-3-1 2-1 0-1z`,RW:`M584 248l2 2-1 2-2 0-1 2-1 0 0-2 0-3 1-1 1 1 1-1z`,SA:`M591 136l3 1 1-1 1-2 1 0 1-1 1-1-4-3 5-2 1-1 3 1 4 3 9 7 4 0 3 0 1 2 2 0 1 3 2 1 0 1 2 1 1 2-1 1 1 1 1 1 0 1 1 1 1 1 1 0 0 1 0 1 2 4 8 1 1 0 1 2-1 7-7 4-8 1-2 2-2 4-1 0-1-1-1 0-3 0-3 0-1 0-1-1-1 2 1 1-1 2-1-2-1-1 0-2-2-1-1-3-1-3-2-2-2-1-2-3 0-3 0-2-2-4-2-1-1-1-1-2 0-1-1-2-1-1-1-2-2-3-2-2-1 0 0-2 0-2 0-1z`,SB:`M947 282l0 2-2 0 0-3 1 1 1 0zM946 279l0 1-2-3 0-3 1 0 0 3 1 2zM944 280l-1 0-2 0-1-1 1-1 2 0 0 1 1 1zM941 273l1 2-2-1-1-2-1-1 1 1 2 1zM935 270l1 1-1 0-1-1-1-1 0-1 2 2z`,SD:`M568 213l-2-2-1-1 0-1 0-2 0-1-2-3 0-1 0-1-1-1 0-2-1-2-1 0 0-1 1-1 0-2 1-1-1-1 1-2 1-3 2 0-1-14 0-2 3 0 0-7 11 0 10 0 11 0 1 4-1 0 1 4 2 4 1 1 1 2-1 2-2 0-1 1 0 3-1 5 1 2-1 3-1 3-1 2-1 3 0 2-2 1 0 4 0 3 0-3-1 0 0-2 0-1-2-2 0-2 0-3-1 0 0 1-2 0 1 1 0 2-1 2-2 3-1 0-2-2-1 1-1 1-1 1-3 0-2-1-1 1-1 0-1-2-1-1-2 0-1 2 0 3-1 1-1 0 2 2z`,SE:`M523 45l1-1 2-2 0-3-1-1-1-3 1-2 2 0 1-1-1-1 3-3 1-2 1-2 2 0 0-1 4 0 0-1 1 0 3 1 3 1 1 4 1 0-3 1-2 2 1 1-3 2-4 2-1 3 2 2 2 1-1 3-2 1-1 4 0 2-3 0-1 2-3 0-1-2-2-3-2-4z`,SI:`M533 80l1 0 2-1 2 0 1 1-2 1 0 1-1 0 0 1-1 0 0-1-1 1-1 0-1-2 1-1z`,SK:`M552 72l0 1-1 1-3-1-1 1-1 1-1 0-1 0 0 1-2 0-1 0-2-1 0-1 1-1 1 0 0-1 1 0 0-1 1 0 1 0 2 1 1-1 1 0 2 0 2 1z`,SL:`M463 210l2-2 0-1 1 0 1-1 2 0 1 1 1 2 0 1 0 1 0 1 1 0-2 2-1 2 0 1-1 1-2-2-2-2 0-1-1-3z`,SN:`M454 193l-1-3-1-1 1-1 1-3 1-2 1-1 1 1 2-1 1 0 2 1 1 1 2 2 2 3 0 2 0 2 1 1 1 2-1 1-2 0-2-1-2 0-5 0 0 1-1 0-2 0 0-3 2 0 1 0 1-1 2 1 1 0 1-1 0-1-1 1-1 0-1-1-1 0-1 1-3 0z`,SO:`M615 250l-1-3 0-14 2-4 1-1 2-1 2-2 4 0 7-12 2-3 1-2 0-2 0-4 0-1 1 0 1-1 1 0 1-1 1 0 1 1-1 2 1 2-1 1 0 4-2 4-1 5-2 5-3 5-3 5-2 3-5 3-2 3-3 5-1 2-1 1z`,SR:`M349 235l-2-1-1 0-1 0-1 1 1 1-1 1-1-1-2-3 0-2-1 0-1-3 1-2-1-1 2-1 1-3 3 1 0-1 3 0 2 1-1 3 0 3 1 2 0 1-1 2 0 2z`,SS:`M586 230l-3-2-1-2-1 1-1 0-1 0-1 0-2-3-1-1-2-2 0-2-2-2-1-2-1-1-1-1-2-2 1 0 1-1 0-3 1-2 2 0 1 1 1 2 1 0 1-1 2 1 3 0 1-1 1-1 1-1 2 2 1 0 2-3 1-2 0-2-1-1 2 0 0-1 1 0 0 3 0 2 2 2 0 1 0 2 1 0 0 3-1 1-1 0-1 2 2 1 1 1 1 2 1 1 2 4-2 2-2 2-1 2-2 0-3 1-1-1-1 1z`,SV:`M256 190l1 1 1 0 0 1 1-1 0 1 1 0-1 2-2 0-1 0-1-1-1 0-1-1 1-1 1-1 1 0z`,SY:`M592 125l0-1 0-1 0-2 1-2 0-1-2 0 0-3 0-1 1-1 0-2 1 0 3-1 1 1 2 0 3-1 1 0 3-1-1 2-1 1 1 3-1 4-5 3-4 4-3-1z`,SZ:`M585 341l-1 2-2 0-1-1 0-2 1-1 0-1 1 0 1 0 0 2 1 1z`,TD:`M564 171l1 14-2 0-1 3-1 2 1 1-1 1 0 2-1 1 0 1 1 0 1 2 0 2 1 1 0 1-2 0-1 2-2 4-3 2-2-1-1 1 0 1-1 1-1 2-4 1 0-1-1 0 0 1-3 1 1-1-1-3-1-1-1-1-1-2 0-2 1 0 1 0 2 0-2-3 0-3 0-2-1-3 0-1-2 0 0-3-1-1 1-5 4-3 0-5 0-8 1-1-1-1 0-1-1-1-1-6 2-2 11 7 11 7z`,TF:`M660 413l1 1 2 1-2 1-3 1 0-2 1-1 1-1z`,TG:`M502 202l0 2 1 1 1 2 0 2 1 0-1 9 1 2-2 1-1-1 0-2-1-2 1-4-1-1 0-3 0-3-1-2 0-1 2 0z`,TH:`M788 190l-3 0-3 0-1 3 1 5-3-2-2 0 0-3-3 0 1 4-1 6-1 3 1 3 2 0 1 3 1 4 2 2 1 0 2 2-1 2-2 0 0-2-2-1-1 0-1-1-1-2-1-2-2-2 0 2-1-2 0-2 1-4 1-4 1-3-2-3 0-2-1-2-2-3-1-2 1-1 1-3-2-2-2-3-2-3 1-1 0-4 2 0 1-1 2-1 1 1 1 2 2 0 0 4 0 3 3-2 1 1 1 0 1-2 2 1 2 3 1 3 3 3 1 4-1 1z`,TJ:`M670 110l0-4-1-2-2-1 0-1 2 0 1-2 0-2 3-1 0 2 1 1 1-1-1 1-3 0 1 2 2 0 4 1 4-1 2 3 2 1 0 1 1 2-2 0-2-1-1 2-1 0-1 1-1-1-1-3 0-1-2-1-1 1 0 2-1 0-1 2-1-1-1 1-1 0z`,TL:`M845 277l1-1 2-1 2 0 1-1 1 1-1 1-3 1-3 1 0-1z`,TM:`M628 95l1-2 2 0 2 1 2 2 2 0 2 0 0-2 1-1 2-2 4 2 1 3 1 0 2 0 1 1 3 3 3 2 2 2 3 1 4 2 0 2-2-1 0 1-2 1 0 2-1 1-2 1 0 1-2 1-2-1-1-3-2 0-4-3-2-1-3-1-2-1-1 1-2 0-1 2-2 0-1-2 0-3-3-1 0-3-1 0 0-3 2 1 2-1-2-2-2-2-1 1 0 3-1-2z`,TN:`M525 133l-2-6-1-2 0-1-3-2 0-2 2-2 0-3 0-3 0-2 3-1 2 0 0 2 2-1-1 2 0 2 1 0-1 3-1 2 0 2 2 0 0 2 1 0 0 3-1 1-1 1-1 1 0 2 0 1-1 1z`,TR:`M612 110l-1 0-1-1-3 0-1 0-3 1-1 0-3 1-2 0-1-1-3 1-1 0 0 2-1 1-1-2 1-1-2 1-2-1-1 2-4 0-2-2-3 0-1 2-1 0-3-2-3 0-1-3-2-2 0-2-1-2 2-3 4 0 0-3 5 1 3-2 2-1 4 0 5 2 4 1 3 0 2 0 2-2 3 0 3 2 0 1 0 1 2 1 1 1-1 1 2 4-1 1 2 3zM564 94l2-1 2 1 1 1 2 1-1 1-2 0-1 1-2 2-1-2 0-1 1-2-1-1z`,TT:`M330 203l2 0 0 3-2 0-1 0 1-1 0-2z`,TW:`M824 154l0 5 0 3-3-3-1-2 1-4 1-3 1 1 1 3z`,TZ:`M594 247l1 0 10 8 0 2 4 4-2 4 0 3 2 1 0 1-1 2 1 1-1 2 1 2 1 4 1 1-2 2-3 1-2 0-1 1-2 1-1 0-3-1-2 0 0-5-1-1-1-1-3-1-1-1-2-1-1-1-1 0-1-5-2-2 0-2 0-2-1-4 2 0 1-1 1-2 0-1 0-1 0-1 0-2 1-2-2-2 1-1 4 0 5 0z`,UA:`M572 63l1 0 1 0 2 0 2 1-1 1 1 1 1 0 2 1 0 1 3 1 1-1 2 2 1 0 4 1 0 2 1 2 0 1-3 0-1 1 1 1-2 0-2 1-2 0-2 2 1 1-1-1-1 0-1 0-1 0-4-1-1-1-2 1 0 1-2 3-1-1-1 1-1-1 1 0 0-1 0-1 0-1 1 0 1 0 1 0-1 0 0-1-1 0 0-2-1 0 0-1-1-1-1 0-2-1-2 0 0 1-1 0-1 0-1 1-1 0-1-1-2 0-1 0-1 1 0-1-2-1 1-1 0-1 1 0-1-1 2-2 1-1 0-1-2-2 1 0 1-1 2 0 2 0 3 1 2 0 1 0 1 0 2 0 1 1 0-2 1 0 2-1z`,UG:`M594 247l-5 0-4 0-1 1-1 1-1-1 0-2 1-2 0-3 1-2 1-2 1-1 1-1-2 0 1-5 1-1 1 1 3-1 2 0 1-2 2 3 0 2 1 4-1 3-1 2-1 2 0 4z`,US:`M216 72l7 0 7 0 2 0 7 0 7 0 7 0 7 0 8 0 7 0 5 0 1-1 1 0-1 2 1 0 1 1 2 0 2 1 2 0 2 0 1 0 3-1 1 2 2 1 1 1 2 1 0 1 1 0 0 1 1 0-1 1 2 1-1 3-1 3-1 2-2 1-1 2 1 1 4-2 2 0 4-2 1-1-1-1 2 0 2 0 2 0 2-2 3-2 2-1 3 0 5 0 1 0 1-1 2-1 1-3 3-2 1 1 1-1 1 1-2 4 1 2 0 1-3 2-3 1-2 1-2 1-1 1-1 2 0 1 1 1 1-2 0 1 0 1-2 1-1-1-2 1-2 0-1 0-2 1 3 0 1 0-4 1-1 0-1 1-1 2-3 3 0-1-1-1 0 2 1 0-1 1-1 2-2 2 1-2-1-1 1-3-1 1 0 3-2-1 2 1-1 3 0 2 0 3-3 2-3 1-2 2-1 1-2 1-1 1-3 2-2 2-2 2-1 2 0 3 0 3 0 2 0 2 0 4 0 2-1 1-1 3-1 0-1 0 0-2-1-1-1-3 0-2 0-2 1-2 0-2-2-3-3 1-1 0-1-2-1-1-3 1-2-1-2 1-2 0 1 1-1 2 0 1-1-1-1 1-2 0-1-2-3 0-1 0-2 0-3 1-3 2-3 2-2 1-1 2-1 2 0 2 0 1-1 0-2 0-1-1-1-2 1-3-1-2-1-2 0-2-1-1-3 0-2 2-2-1-1-1 0-2 0-1-1-2-1-1 0-1-5 0 0 1-3 0-5 0-5-2-3-2 1-1-4 1-3 0 1-2-1-2-1 0 0-1-1 0-1-1-2 0 0-1 0-2-1-3 1-5 0-1 0-1-1-2 2-3-1-2 2-2 2-3 1-3 3-3 2-2 2-3 3-4 1-3 0-1 1-1 2 1 0 3 1-1 1-2 1-3zM81 169l1 1 0 1-2 1-1 1-1 0 1-1 0-2 1-1 0-1 1 1zM80 167l-1 0 0-1-1 0 1 0 1 0 0 1zM79 165l-1 0-1 0 2 0zM76 163l0 2-1 0 0-1 1-1zM73 161l0 1-1 1 0-1 1-1zM153 41l1 1-1 1-2 0 0-1-1 0 3-1zM173 47l1 1-3 2-4 1 0-1 1-1 3-1 2-1zM233 21l-10 7-17 14 2 0 1 0 0 1 0 2 4-1 3-1 0 1 0 1 1 1-1 2-1 3 2 2-2 2-2 1 0-1-1-1 1-2 0-2 1-3-2 0-3 0-2-1-1-3-1 0-3-1-3 0-3-1-1-1-3 1-2 1-1 1-3 0-3 1-3 0 1-1 4-2 4-1 0-1-5 2-3 1-5 2 0 1-5 2-3 1-3 1-2 1-5 1-3 2-3 1-2 0-3 0-3 1-3 1-5 1 1-1 4-1 3-1 4-1 3 0 2-1 5-2 1-1 3-1 3-1 3-2-4 1 0-1-2 1 0-1-2 1 1-1-3 1-1 0 1-2 2-1 0-1-4 1 0-1 0-1 2-1 0-1 2-2 4-1 2-1 2 0 1 0 3-1 2 0 2-1 1-1 3-1-2 0-2 1-2 0-1 0-3 0-2-1 1-1-1-1 4-1 6-1 2 0-2 1 4 0 1-1-1-1 0-1 0-1-2-1 3-1 3 0 4-1 2 0 4-1 2-1 5 0 2 0 4-1 2 0 0 1 2 0 3 0-1 0 2 0 2 0 3 1 4 0 1 0 3 0 2 1 1 0zM154 33l1 1 1-1 1 1 2 0-1 1-2 0-1 0 0-1-2 0 1-1z`,UY:`M350 354l2-1 3 3 1 0 3 2 3 2 2 2-1 2 1 2-1 2-2 2-2-1-2 0-2-1-2 0-2-2 0-2 0-1-1-3 0-4 0-2z`,UZ:`M637 96l-4-12 6-1 5 2 2 1 3 3 3 0 4-1 4 3 0 3 2 0 1 3 3 0 1 1 1 0 1-2 2-2 1-1 1 0-1 2 2 2 1-1 4 1-2 3-2-1-1 1-1-1 0-2-3 1 0 2-1 2-2 0 0 1 2 1 1 2 0 4-2-1-2 0 0-2-4-2-3-1-2-2-3-2-3-3-1-1-2 0-1 0-1-3-4-2-2 2-1 1 0 2-2 0z`,VE:`M332 224l0 1-1 1-3 2-2 0-1 1-2-1-2 0-1 0 1 1 0 3 1 2 2 0 0 1-2 1 0 2-1 0-3 1 0 1-2 1-2-2-1-4 0-1-1-1 1-2 0-1-1-1 0-2 0-3 1-1 0-2-1-1-1 0-2 0-1 1-2-4-2 0-3 0-1-1 0-1 0-2 0-1-1-1 0-2-1 0 1-2 0-3 1-1 1-1 1-2 2-1 0 1-2 1 1 1 0 2-2 3 1 2 1 0 1-2-1-2 0-3 4-1 0-2 1-1 1 3 1 0 2 2 0 1 3 0 3 0 1 1 2 1 2-1 0-1 3-1 4 0-3 1 1 2 2 1 2 1 0 3 2 0 1 1-2 2-1 2 1 1-1 1-1 0 0 2-1 1 2 3z`,VN:`M788 204l2-1 3-1-2-2 4-3 0-4-1-3 0-3-1-3-3-2-2-4-3-4-4-2 0-1 2-1-2-3-3 0-2-4-3-3 2 0 2 0 2-1 2-2 2 2 2 0 0 2 2 2 3 1-3 3-1 3 0 3 3 4 3 4 3 3 2 3 3 6 0 7-2 2-2 3-2 3-3 3-2-2 1-3-2-2z`,VU:`M956 302l1 3-1 0 0-2 0-1zM955 302l0-1 1-3 1 1-1 3 0-1-1 1z`,XK:`M550 94l0-1-1 0 0-1 0-1 1 0 0-1 1 0 0 1 1 0 0 1 1 0-1 0 0 1-2 1z`,YE:`M641 173l2 6 2 3-2 0-1 2 0 1-2 2-4 2-3 2-1 0-1 0-1 2-2 0-2 1-1 0 0 1-1 0 0 1-2 0-1 0-2 0 0-2-1-2 0-1-1-3-1-1 1 0-1-2 1-1 0-1 1-2-1-1 1-2 1 1 1 0 3 0 3 0 1 0 1 1 1 0 2-4 2-2 8-1z`,ZA:`M543 348l1-2 1 1 0 2 2 0 1 1 2-1 2-1 1-13 1 0 1 3 0 3 0 1 2-1 1-1 2-1 0-2 2-1 1 1 1 1 2 0 2-1 0-1 1-2 1 0 1-1 1-3 3-2 4-3 1 0 1 1 1-1 2 1 0 5 1 2-1 4 0 1-1 0-1 0 0 1-1 1 0 2 1 1 2 0 1-2 1 1 0 2-1 3-1 2-2 1 0 1-2 2-1 1-1 3-4 3-2 2-2 2-3 1-1 0 0 1-2 0-1 1-3-1-1 0-1 0-3 1-2 1-2 1-1 0-1-1-1 0-1-1 0-1 0-2-1-2 1-1 0-2-2-4-1-3-1-4z`,ZM:`M585 275l1 0 1 1 2 1 1 1 2 1 0 4 0 1-1 3 0 3 0 1-1 3 1 1-8 3 0 3-2 1-2 1 0 1-1 1-3 3-2 2-1 0-3-1-1 0-1-1-2 0-2 0-2-2-1-3 0-12 6 0 0-1 0-1 0-2 0-2 0-1 1 0 0 1 1 0 2 1 1 1 2 1 2-1 0 2 2 0 1 2 1 2 2 0 0-4-2-1-1-1 0-3 1-5 0-1 0-3 1 0 4-1 1 1z`,ZW:`M584 326l-2-1-1 1-1-1-1 0-2-2-2 0 0-3 0-1-1 0-3-4-1-2-1-1 0-3 3 1 1 0 2-2 3-3 1-1 0-1 2-1 2-1 0 1 2 0 1 1 1 1 1 0 1 1 0 5-1 3 0 2 0 1 0 3-1 2-3 5z`},hi=o`
  :host {
    display: block;
    width: 100%;
    box-sizing: border-box;
    min-width: 0;
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-family: var(--md-sys-typescale-body-medium-font, Roboto, system-ui, sans-serif);
    font-size: var(--md-sys-typescale-body-small-size, 0.75rem);
    line-height: var(--md-sys-typescale-body-small-line-height, 1rem);
    --fill: var(--lit-material-world-map-fill, var(--lit-material-chart-series-1, light-dark(#2a78d6, #3987e5)));
    --empty: var(--lit-material-world-map-empty, var(--md-sys-color-surface-container-highest, light-dark(#e6e0e9, #36343b)));
    --border: var(--lit-material-world-map-border, var(--md-sys-color-surface, light-dark(#fef7ff, #141218)));
    --muted: var(--md-sys-color-on-surface-variant, light-dark(#49454f, #cac4d0));
    --focus: var(--md-sys-color-primary, light-dark(#6750a4, #d0bcff));
  }

  :host([hidden]) {
    display: none;
  }

  .figure {
    margin: 0;
    display: grid;
    gap: 8px;
  }

  .frame {
    position: relative;
    border-radius: 4px;
  }
  .frame:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 4px;
  }

  svg {
    display: block;
    width: 100%;
    height: auto;
  }

  .country {
    fill: var(--empty);
    stroke: var(--border);
    stroke-width: 0.5;
    stroke-linejoin: round;
  }
  .country.on {
    fill: color-mix(in oklab, var(--fill) var(--level), var(--empty));
  }
  :host([selectable]) .country.on {
    cursor: pointer;
  }
  .country.active,
  .country.selected {
    stroke: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    stroke-width: 1.5;
  }
  .country.selected {
    stroke: var(--focus);
    stroke-width: 2;
  }

  .tooltip {
    position: absolute;
    transform: translate(-50%, calc(-100% - 6px));
    pointer-events: none;
    padding: 6px 8px;
    border-radius: var(--md-sys-shape-corner-small, 8px);
    background: var(--md-sys-color-inverse-surface, light-dark(#322f35, #e6e0e9));
    color: var(--md-sys-color-inverse-on-surface, light-dark(#f5eff7, #322f35));
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .tooltip.corner {
    inset-block-start: 0;
    inset-inline-start: 0;
    transform: none;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .ramp {
    flex: 0 1 160px;
    height: 8px;
    border-radius: 4px;
    background: linear-gradient(
      to right,
      color-mix(in oklab, var(--fill) 18%, var(--empty)),
      color-mix(in oklab, var(--fill) 90%, var(--empty))
    );
  }
  :host(:dir(rtl)) .ramp {
    transform: scaleX(-1);
  }

  .elsewhere {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 12px;
    color: var(--muted);
  }
  .elsewhere-label {
    font-weight: 600;
  }
  .place {
    font: inherit;
    color: var(--md-sys-color-on-surface, light-dark(#1d1b20, #e6e0e9));
    font-variant-numeric: tabular-nums;
  }
  button.place {
    min-height: 24px;
    padding: 0 8px;
    border: 1px solid var(--md-sys-color-outline-variant, light-dark(#cac4d0, #49454f));
    border-radius: var(--md-sys-shape-corner-small, 8px);
    background: transparent;
    cursor: pointer;
  }
  button.place[aria-pressed="true"] {
    border-color: var(--focus);
    background: color-mix(in oklab, var(--focus) 12%, transparent);
  }
  button.place:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
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
    .country {
      fill: Canvas;
      stroke: CanvasText;
    }
    .country.on {
      /* No colour scale in forced colours: shaded or not, values are in the readout and the table. */
      fill: GrayText;
    }
    .country.selected {
      fill: Highlight;
      fill-opacity: 1;
    }
    .ramp {
      background: CanvasText;
    }
  }
`,K=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},gi=18,_i=(e,t)=>e>0&&t>0?Math.round(gi+72*Math.sqrt(Math.min(1,e/t))):0,vi=5,q=class extends y{constructor(){super(...arguments),this.data={},this.label=``,this.unit=``,this.selectable=!1,this.selected=``,this.localize=new li(this),this.active=``,this.tipAt=null,this.namesLang=``}static{this.styles=hi}countryName(e){let t=this.localize.lang;if(this.names===void 0||this.namesLang!==t){this.namesLang=t;try{this.names=new Intl.DisplayNames([t,`en`],{type:`region`})}catch{this.names=null}}try{return this.names?.of(e)??e}catch{return e}}get entries(){let e=new Map;for(let[t,n]of Object.entries(this.data??{})){if(typeof n!=`number`||!Number.isFinite(n)||n<0)continue;let r=t.trim().toUpperCase();r&&e.set(r,(e.get(r)??0)+n)}return[...e].map(([e,t])=>({code:e,value:t,name:this.countryName(e)})).sort((e,t)=>t.value-e.value||e.name.localeCompare(t.name))}formatValue(e){return this.format?this.format(e):this.localize.number(e,Math.abs(e)>=1e4?{notation:`compact`,maximumFractionDigits:1}:{maximumFractionDigits:2})}text(e){let t=this.formatValue(e);return this.unit?this.localize.term(`worldMap.valueWithUnit`,`{value} {unit}`,{value:t,unit:this.unit}):t}readout(e,t){let n=this.countryName(e);return t===void 0?this.localize.term(`worldMap.nameNoData`,`{name}: no data`,{name:n}):this.localize.term(`worldMap.nameValue`,`{name}: {value}`,{name:n,value:this.text(t)})}show(e){this.active=e;let t=this.renderRoot.querySelector(`path[data-code="${e}"]`)?.getBBox?.(),[,,n,r]=pi.split(` `).map(Number);this.tipAt=t?{x:Math.min(88,Math.max(12,(t.x+t.width/2)/n*100)),y:t.y/r*100}:null}hide(){this.active=``,this.tipAt=null}pick(e,t){if(!this.selectable||t===void 0)return;let n=this.selected===e;this.selected=n?``:e,this.dispatchEvent(new CustomEvent(`change`,{detail:{code:this.selected,value:n?null:t},bubbles:!0,composed:!0}))}onKey(e,t){if(e.key===`Escape`){this.hide();return}let n=t.findIndex(e=>e.code===this.active);if((e.key===`Enter`||e.key===` `)&&n>=0){e.preventDefault(),this.pick(t[n].code,t[n].value);return}let r=ci(this),i=e=>Math.min(t.length-1,e+1),a=e=>e<0?t.length-1:Math.max(0,e-1),o={ArrowDown:i,ArrowUp:a,ArrowRight:r?a:i,ArrowLeft:r?i:a,Home:()=>0,End:()=>t.length-1}[e.key];o&&t.length!==0&&(e.preventDefault(),this.show(t[o(n)].code))}renderCountry(e,t,n,r){let i=n.get(e),a=i!==void 0&&i>0;return g`<path
      d=${t}
      data-code=${e}
      class=${[`country`,a?`on`:``,e===this.active?`active`:``,e===this.selected?`selected`:``].join(` `)}
      style=${fi({"--level":a?`${_i(i,r)}%`:null})}
      @pointerenter=${()=>this.show(e)}
      @click=${()=>this.pick(e,i)}
    ></path>`}render(){let e=this.entries,t=this.label||this.localize.term(`worldMap.worldMap`,`World map`),n=new Map(e.map(e=>[e.code,e.value])),r=e[0]?.value??0,i=e.filter(e=>!(e.code in mi)),a=this.localize.term(`worldMap.listSeparator`,`; `),o=e.length?this.localize.term(`worldMap.labelTop`,`{label}. Highest: {top}`,{label:t,top:e.slice(0,vi).map(e=>this.readout(e.code,e.value)).join(a)}):this.localize.term(`worldMap.nameNoData`,`{name}: no data`,{name:t}),s=this.active?this.readout(this.active,n.get(this.active)):``,c=s&&this.active===this.selected?this.localize.term(`worldMap.readoutSelected`,`{readout}, selected`,{readout:s}):s,l=e.filter(e=>e.value>0).at(-1)?.value??0;return h`
      <figure class="figure">
        <div
          class="frame"
          role="img"
          aria-label=${o}
          tabindex="0"
          @keydown=${t=>this.onKey(t,e)}
          @pointerleave=${()=>this.hide()}
          @blur=${()=>this.hide()}
        >
          <svg part="map" viewBox=${pi} aria-hidden="true">
            ${Object.entries(mi).map(([e,t])=>this.renderCountry(e,t,n,r))}
          </svg>
          ${this.active?h`<div
                class="tooltip ${this.tipAt?``:`corner`}"
                part="tooltip"
                aria-hidden="true"
                style=${fi(this.tipAt?{left:`${this.tipAt.x}%`,top:`${this.tipAt.y}%`}:{})}
              >${s}</div>`:v}
        </div>
        ${r>0?h`<div class="legend" part="legend" aria-hidden="true">
              <span>${this.formatValue(l)}</span><span class="ramp"></span><span>${this.text(r)}</span>
            </div>`:v}
        ${i.length?h`<div class="elsewhere" part="elsewhere">
              <span class="elsewhere-label">${this.localize.term(`worldMap.notOnMap`,`Not on the map`)}</span>
              ${i.map(e=>this.selectable?h`<button
                      type="button"
                      class="place"
                      aria-pressed=${e.code===this.selected?`true`:`false`}
                      @click=${()=>this.pick(e.code,e.value)}
                    >${this.readout(e.code,e.value)}</button>`:h`<span class="place">${this.readout(e.code,e.value)}</span>`)}
            </div>`:v}
      </figure>
      <span class="visually-hidden" role="status">${c}</span>
      ${e.length?h`<table class="visually-hidden">
            <caption>${t}</caption>
            <thead>
              <tr>
                <th scope="col">${this.localize.term(`worldMap.country`,`Country`)}</th>
                <th scope="col">${this.unit?this.localize.term(`worldMap.valueInUnit`,`Value ({unit})`,{unit:this.unit}):this.localize.term(`worldMap.value`,`Value`)}</th>
              </tr>
            </thead>
            <tbody>
              ${e.map(e=>h`<tr><th scope="row">${e.name}</th><td>${this.formatValue(e.value)}</td></tr>`)}
            </tbody>
          </table>`:v}
    `}};K([x({type:Object})],q.prototype,`data`,void 0),K([x()],q.prototype,`label`,void 0),K([x()],q.prototype,`unit`,void 0),K([x({type:Boolean,reflect:!0})],q.prototype,`selectable`,void 0),K([x()],q.prototype,`selected`,void 0),K([x({attribute:!1})],q.prototype,`format`,void 0),K([S()],q.prototype,`active`,void 0),K([S()],q.prototype,`tipAt`,void 0),q=K([b(`lit-material-world-map`)],q);var J=[[`overview`,`Overview`],[`visitors`,`Visitors`],[`pages`,`Pages`],[`sources`,`Sources`],[`audience`,`Audience`],[`behaviour`,`Behaviour`]],yi=[[`people`,`Humans`],[`apps`,`Apps`],[`ai`,`AI agents`],[`bots`,`Bots`],[`all`,`All`]],bi={people:`Humans`,apps:`Apps`,ai:`AI agents`,bots:`Bots`},xi=[[`today`,`Today`,0],[`7d`,`7 days`,6],[`30d`,`30 days`,29]],Si={page:`Page`,country:`Country`,channel:`Channel`,utm_source:`Source`,utm_campaign:`Campaign`,browser:`Browser`,os:`System`,device:`Device`},Ci={direct:`Direct`,search:`Search`,social:`Social`,ai:`AI assistants`,store:`App stores`,referral:`Other sites`,campaign:`Campaigns`,"(none)":`Direct`},wi={ios:`iOS`,ipados:`iPadOS`,macos:`macOS`,apple:`iOS/macOS`,android:`Android`,windows:`Windows`,linux:`Linux`,chrome:`Chrome`,edge:`Edge`,firefox:`Firefox`,safari:`Safari`,web:`Web`,other:`Other`},Ti={people:`people`,apps:`apps`,bots:`bots, tools`,ai:`AI`,owner:`you`,undecided:`undecided`},Ei=e=>e.split(`	`);function Di(e){let t=Math.max(0,Math.round(Date.now()/1e3-e));return t<60?`just now`:t<3600?`${Math.floor(t/60)} min ago`:t<86400?`${Math.floor(t/3600)} h ago`:new Date(e*1e3).toLocaleDateString([],{month:`short`,day:`numeric`})}var Oi={desktop:`Desktop`,mobile:`Phone`,tablet:`Tablet`},ki={phone:`Phone (< 600 px)`,tablet:`Tablet (600–1023 px)`,laptop:`Laptop (1024–1439 px)`,wide:`Wide (≥ 1440 px)`,"(unknown)":`Unknown`},Ai=(e,t)=>{let n=new Date(e);return t===36e5?(n.setMinutes(n.getMinutes()+30,0,0),n.setMinutes(0)):(n.setHours(n.getHours()+12,0,0,0),n.setHours(0)),n},ji=e=>Ai(e,36e5).toLocaleTimeString([],{hour:`numeric`}),Mi=e=>Ai(e,864e5).toLocaleDateString([],{month:`short`,day:`numeric`}),Ni=50,Y=(e=0)=>new Date(Date.now()-e*864e5).toISOString().slice(0,10),X=e=>(e??0).toLocaleString(),Pi=e=>e>=1?Gr(Math.round(e)*1e3,{unitDisplay:`narrow`}):`0s`,Fi=e=>`${Math.round(e*100)}%`,Ii=null;try{Ii=new Intl.DisplayNames([navigator.language||`en`],{type:`region`})}catch{}var Li=e=>{if(e===`(unknown)`)return`Unknown`;if(e===`(other)`)return`Other`;try{return Ii?.of(e)??e}catch{return e}};function Ri(e,t,{points:n=!1,lowerIsBetter:r=!1}={}){if(t==null||!n&&!t)return null;let i=n?(e-t)*100:(e-t)/t*100,a=Math.round(i);if(a===0)return{text:n?`±0 pt`:`±0%`,tone:`flat`};let o=a>0;return{text:`${o?`+`:`−`}${Math.abs(a)}${n?` pt`:`%`}`,tone:o===r?`bad`:`good`}}var zi=class extends y{static properties={tab:{state:!0},group:{state:!0},preset:{state:!0},from:{state:!0},to:{state:!0},host:{state:!0},filters:{state:!0},metric:{state:!0},compare:{state:!0},data:{state:!0},live:{state:!0},query:{state:!0},notice:{state:!0},off:{state:!0},customOpen:{state:!0},owner:{state:!0},loading:{state:!0},failed:{state:!0},shown:{state:!0},visitFilter:{state:!0},tiersOpen:{state:!0}};constructor(){super(),this.tab=`overview`,this.group=`people`,this.preset=`7d`,this.from=Y(6),this.to=``,this.host=``,this.filters={},this.metric=`visitors`,this.compare=!0,this.data=void 0,this.live=null,this.query=null,this.notice=``,this.off=!1,this.customOpen=!1,this.owner=null,this.loading=!1,this.failed=``,this.shown={},this.visitFilter=``,this.tiersOpen=!1;try{let e=JSON.parse(localStorage.getItem(`rws-traffic`)??`{}`);J.some(([t])=>t===e.tab)&&(this.tab=e.tab),typeof e.compare==`boolean`&&(this.compare=e.compare),yi.some(([t])=>t===e.group)&&(this.group=e.group)}catch{}}createRenderRoot(){return this}connectedCallback(){super.connectedCallback(),this.reload(),this.timer=setInterval(()=>{(!this.to||this.to>=Y(0))&&this.reload({quiet:!0})},1e4)}disconnectedCallback(){super.disconnectedCallback(),clearInterval(this.timer)}remember(){try{localStorage.setItem(`rws-traffic`,JSON.stringify({tab:this.tab,compare:this.compare,group:this.group}))}catch{}}get filtered(){return Object.keys(this.filters).length>0}range(){let e=new URLSearchParams({from:this.from});return this.to&&e.set(`to`,this.to),this.host&&e.set(`host`,this.host),e}async reload({quiet:e=!1}={}){e||(this.loading=!0);try{let e=this.range(),[t,n]=await Promise.all([G(`/traffic?${e}`),G(`/traffic/live?${this.host?`host=${encodeURIComponent(this.host)}`:``}`).catch(()=>null)]);this.data=t,this.live=n,this.off=!1,this.failed=``,this.filtered&&await this.runQuery()}catch(t){t instanceof W&&t.status===404?this.off=!0:e&&this.data||(this.failed=t instanceof W?t.message:`The admin API didn't answer.`)}finally{this.loading=!1}}async runQuery(){let e=this.periodDays(),t=this.range();t.get(`to`)||t.set(`to`,Y(0));for(let[e,n]of Object.entries(this.filters))if(e===`page`){let[e,...r]=n.split(`/`);t.set(`path`,`/${r.join(`/`)}`),this.host||t.set(`host`,e)}else t.set(e,n);let n=new URLSearchParams(t),r=Date.parse(t.get(`from`));n.set(`to`,new Date(r-864e5).toISOString().slice(0,10)),n.set(`from`,new Date(r-e*864e5).toISOString().slice(0,10));try{let[e,r]=await Promise.all([G(`/traffic/query?${t}`),G(`/traffic/query?${n}`).catch(()=>null)]);this.query={now:e,before:r}}catch(e){this.query=null,this.notice=e.message??String(e)}}periodDays(){let e=this.to||Y(0);return Math.round((Date.parse(e)-Date.parse(this.from))/864e5)+1}setPreset(e){let t=xi.find(([t])=>t===e);this.preset=e,this.customOpen=!1,this.notice=``,this.from=Y(t[2]),this.to=``,this.reload()}applyCustom(e){e.preventDefault();let t=this.querySelector(`#traffic-from`)?.value,n=this.querySelector(`#traffic-to`)?.value;t&&(n&&n<t?this.notice=`The start date is after the end date.`:(this.preset=`custom`,this.from=t,this.to=n&&n<Y(0)?n:``,this.customOpen=!1,this.notice=``,this.reload()))}setHost(e){this.host=e,this.reload()}pick(e,t){this.data?.store?(this.notice=``,this.filters={...this.filters,[e]:t},this.tab=`overview`,this.runQuery()):this.notice=`Filtering by ${Si[e].toLowerCase()} needs the traffic database: set [traffic] database_url_env.`}unpick(e){let t={...this.filters};delete t[e],this.filters=t,this.filtered?this.runQuery():this.query=null}setTab(e){this.tab=e,this.notice=``,this.shown={},this.remember()}async showOwner(){if(this.owner)this.owner=null;else try{this.owner=await G(`/traffic/owner`)}catch(e){this.notice=e.message??String(e)}}setGroup(e){this.group=e,e===`people`?this.metric===`requests`&&(this.metric=`pages`):(this.tab=`overview`,this.metric=`requests`),this.remember()}pageName(e){if(e===`(other)`)return`Other pages`;if(!(this.host||(this.data?.hosts?.length??0)<=1))return e;let t=e.indexOf(`/`);return t>=0?e.slice(t):e}label(e,t){if(t===`(other)`)return`Other`;switch(e){case`page`:return this.pageName(t);case`country`:return Li(t);case`channel`:return Ci[t]??t;case`device`:return Oi[t]??t;default:return t}}ranked(e,{dim:t=null,unit:n=``,limit:r=0,value:i=e=>X(e[1]),flag:a=!1,pages:o=!1}={}){let s=r?e.slice(0,r):e;if(!s.length)return h`<p class="t-none">Nothing in this period.</p>`;let c=Math.max(...s.map(e=>e[1]),1),l=t&&t in Si&&!(t in this.filters)&&!!this.data?.store;return h`<ol class="t-rank" aria-label=${n}>
      ${s.map(([e,n,...r])=>{let s=t?this.label(t,e):o?this.pageName(e):e,u=h`<span class="t-bar" style=${`width:${n/c*100}%`}></span>
          <span class="t-name" title=${e}>${a&&e.length===2?h`<span class="t-code">${e}</span>`:v}${s}</span>
          <span class="t-num">${i([e,n,...r])}</span>`;return h`<li>${l&&e!==`(other)`&&e!==`(unknown)`?h`<button class="t-row" title=${`Filter by ${s}`} @click=${()=>this.pick(t,e)}>${u}</button>`:h`<div class="t-row">${u}</div>`}</li>`})}
    </ol>`}card(e,t,n={},r=null){let i=n.limit??(r?6:this.shown[e]??Ni);return h`<section class="t-card" aria-label=${e}>
      <header><h3>${e}</h3>
        ${r&&t.length>6?h`<button class="t-more" @click=${()=>this.setTab(r)}>All ${t.length} →</button>`:n.hint?h`<span class="t-hint">${n.hint}</span>`:v}</header>
      ${this.ranked(t,{...n,limit:i})}
      ${!r&&!n.limit&&t.length>i?h`<button class="t-more t-show" @click=${()=>this.shown={...this.shown,[e]:i+Ni}}>Show ${Math.min(Ni,t.length-i)} more of ${t.length-i}</button>`:v}
    </section>`}numbers(){let e=this.data;if(this.query){let{now:e,before:t}=this.query;return{visitors:[e.visitors,t?.visitors],pages:[e.views,t?.views],sessions:[e.sessions,t?.sessions],bounce:[e.sessions?e.bounces/e.sessions:0,t?.sessions?t.bounces/t.sessions:null],duration:[null,null]}}let t=e.sessions??{},n=e.previous_sessions??{},r=(e.previous?.people?.pages??0)>0||(n.sessions??0)>0;return{visitors:[e.totals.people.visitors,r?e.previous.people.visitors:null],pages:[e.totals.people.pages,r?e.previous.people.pages:null],sessions:[t.sessions??0,r?n.sessions??0:null],bounce:[t.sessions?t.bounces/t.sessions:0,n.sessions?n.bounces/n.sessions:null],duration:[t.sessions?t.duration/t.sessions:0,n.sessions?n.duration/n.sessions:null]}}hasEarlier(){let e=this.data;return e?this.query?!!this.query.before:this.group===`people`?(e.previous?.people?.pages??0)>0||(e.previous_sessions?.sessions??0)>0:Object.values(e.previous??{}).some(e=>(e?.requests??0)>0):!1}chartable(e){return e===`pages`?!0:e===`duration`?!1:this.query?e===`visitors`:this.data.bucket===`day`}chartSeries(){let e=this.data,t=this.chartable(this.metric)?this.metric:`pages`,n={pages:`Page views`,visitors:`Visitors`,sessions:`Sessions`,bounce:`Bounce rate`},r,i=null;if(this.query){let e=t===`visitors`?`visitors`:`views`;r=this.query.now.series.map(t=>({x:t.t*1e3,y:t[e]}));let n=Date.parse(this.from);if(this.query.before){let t=n-this.periodDays()*864e5;i=this.query.before.series.map(r=>({x:r.t*1e3-t+n,y:r[e]}))}}else if(t===`pages`)r=e.series.map(e=>({x:e.t*1e3,y:e.pages??0})),i=(e.previous_series??[]).map(e=>({x:e.t*1e3,y:e.pages??0}));else{let n=(e,n)=>{let r=n?e[`previous_${t===`bounce`?`sessions`:t}`]:e[t===`bounce`?`sessions`:t];if(t!==`bounce`)return r;let i=n?e.previous_bounces:e.bounces;return r?Math.round(i/r*100):0};r=(e.days??[]).map(e=>({x:e.t*1e3,y:n(e,!1)})),i=(e.days??[]).map(e=>({x:e.t*1e3,y:n(e,!0)}))}let a=[{name:n[t],points:r}];return this.compare&&i&&i.some(e=>e.y>0)&&a.push({name:`Period before`,points:i}),{series:a,unit:t===`bounce`?`%`:n[t].toLowerCase()}}hostOptions(e,t){let n=e=>h`<option value=${e} ?selected=${this.host===e}>${e===`other`?`Unknown hosts`:e}</option>`,r=e.filter(e=>t.includes(e)),i=e.filter(e=>!t.includes(e));return!r.length||!i.length||i.length===1&&i[0]===`other`?e.map(n):h`<optgroup label="Sites">${r.map(n)}</optgroup><optgroup label="Other hosts">${i.map(n)}</optgroup>`}renderBar(){let e=this.data,t=e?.hosts??[];return h`<div class="t-bar-top">
      <div class="t-seg" role="group" aria-label="Period">
        ${xi.map(([e,t])=>h`<button aria-pressed=${this.preset===e} @click=${()=>this.setPreset(e)}>${t}</button>`)}
        <button aria-pressed=${this.preset===`custom`} aria-expanded=${this.customOpen} @click=${()=>this.customOpen=!this.customOpen}>
          ${this.preset===`custom`?`${this.from} – ${this.to||`today`}`:`Custom`}
        </button>
      </div>
      <div class="t-seg" role="group" aria-label="Who">
        ${yi.map(([e,t])=>h`<button aria-pressed=${this.group===e} @click=${()=>this.setGroup(e)}>${t}</button>`)}
      </div>
      ${t.length>1?h`<label class="t-select"><span class="t-sr">Site</span>
            <select @change=${e=>this.setHost(e.target.value)}>
              <option value="" ?selected=${!this.host}>All sites</option>
              ${this.hostOptions(t,e?.configured??[])}
            </select></label>`:v}
      <label class="t-check"><input type="checkbox" .checked=${this.compare} @change=${e=>{this.compare=e.target.checked,this.remember()}}> Compare</label>
      ${this.compare&&e&&!this.hasEarlier()?h`<span class="t-hint" title="Counts are kept for retention_days (30 by default); the traffic database keeps more">No earlier period kept to compare with</span>`:v}
      <span class="t-spacer"></span>
      ${this.live?h`<span class="t-live" title=${this.live.pages.map(([e,t])=>`${this.pageName(e)}: ${t}`).join(`
`)||`Nobody right now`}>
            <span class="t-dot" data-on=${this.live.visitors>0}></span>${X(this.live.visitors)} in the last 5 min</span>`:v}
    </div>
    ${this.customOpen?h`<form class="t-custom" @submit=${e=>this.applyCustom(e)} @keydown=${e=>e.key===`Escape`&&(this.customOpen=!1)}>
          <label>From <input id="traffic-from" type="date" .value=${this.from} max=${Y(0)} min=${Y(89)}></label>
          <label>To <input id="traffic-to" type="date" .value=${this.to||Y(0)} max=${Y(0)} min=${Y(89)}></label>
          <lit-material-button type="submit" variant="tonal">Apply</lit-material-button>
          <span class="t-hint">${e?.store?`Up to 90 days; any range with the database in the filter results.`:`Counts are kept for the configured retention_days (30 by default).`}</span>
        </form>`:v}
    ${this.filtered||this.host?h`<div class="t-chips" aria-label="Filters">
          ${this.host?h`<button class="t-chip" @click=${()=>this.setHost(``)}>Site: ${this.host===`other`?`Unknown hosts`:this.host} <span aria-hidden="true">✕</span></button>`:v}
          ${Object.entries(this.filters).map(([e,t])=>h`<button class="t-chip" @click=${()=>this.unpick(e)}>${Si[e]}: ${this.label(e,t)} <span aria-hidden="true">✕</span></button>`)}
          ${this.filtered?h`<button class="t-chip t-clear" @click=${()=>{this.filters={},this.query=null}}>Clear filters</button>`:v}
        </div>`:v}
    ${this.notice?h`<p class="t-notice" role="status">${this.notice} <button class="t-more" @click=${()=>this.notice=``}>Dismiss</button></p>`:v}
    ${e&&!e.store&&this.group===`people`?h`<p class="t-foot t-nodb">Rows filter the page with the traffic database: <a href="https://rws8.tech/docs/deployment/traffic/" target="_blank" rel="noopener">how to turn it on</a>.</p>`:v}`}renderKpis(){let e=this.numbers(),t=[[`visitors`,`Visitors`,X(e.visitors[0]),Ri(e.visitors[0],e.visitors[1]),`Each person counted once a day`],[`pages`,`Page views`,X(e.pages[0]),Ri(e.pages[0],e.pages[1]),`Pages people opened`],[`sessions`,`Sessions`,X(e.sessions[0]),Ri(e.sessions[0],e.sessions[1]),`Page views 30 minutes or less apart`],[`bounce`,`Bounce rate`,Fi(e.bounce[0]),Ri(e.bounce[0],e.bounce[1],{points:!0,lowerIsBetter:!0}),`Sessions of one page view`],[`duration`,`Session length`,e.duration[0]===null?`—`:Pi(e.duration[0]),e.duration[0]===null?null:Ri(e.duration[0],e.duration[1]),`First page view to last, on average`]],n=this.chartable(this.metric)?this.metric:`pages`;return h`<div class="t-kpis" role="group" aria-label="Key numbers">
      ${t.map(([e,t,r,i,a])=>{let o=this.chartable(e);return h`<button class="t-kpi" aria-pressed=${o&&n===e} ?disabled=${!o} title=${a}
          @click=${()=>this.metric=e}>
          <span class="t-kpi-label">${t}</span>
          <span class="t-kpi-value">${r}</span>
          <span class="t-kpi-delta" data-tone=${i?.tone??`none`}>${this.compare&&i?i.text:` `}</span>
        </button>`})}
    </div>`}renderChart(){let{series:e,unit:t}=this.chartSeries(),n=!this.query&&this.data.bucket===`hour`&&this.chartable(this.metric)&&this.metric===`pages`;return h`<section class="t-chart" aria-label="Chart">
      <lit-material-line-chart label=${`${e[0].name} per ${n?`hour`:`day`}`} unit=${t} height="240" .series=${e}
        .formatX=${n?ji:Mi}></lit-material-line-chart>
    </section>`}lists(){let e=this.data;if(this.query){let e=this.query.now;return{pages:e.pages,channels:e.channels,referrers:e.referrers,utm_source:e.utm_source,utm_campaign:e.utm_campaign,utm_medium:[],countries:e.countries,browsers:e.browsers,os:e.os,devices:e.devices,entry:[],exit:[],not_found:[]}}return{pages:e.people_pages,channels:e.channels??[],referrers:e.referrers,utm_source:e.utm_source??[],utm_medium:e.utm_medium??[],utm_campaign:e.utm_campaign??[],countries:e.countries??[],browsers:e.browsers??[],os:e.os??[],devices:e.devices??[],entry:e.entry_pages??[],exit:e.exit_pages??[],not_found:e.not_found}}renderOverview(){let e=this.lists(),t=this.live;return h`
      ${this.renderKpis()}
      ${this.query?v:this.renderTiers()}
      ${this.renderChart()}
      <div class="t-cards">
        ${this.card(`Top pages`,e.pages,{dim:`page`,unit:`views`},`pages`)}
        ${this.card(`Sources`,e.channels,{dim:`channel`,unit:`visits`},`sources`)}
        <section class="t-card t-wide" aria-label="Countries">
          <header><h3>Countries</h3>${e.countries.length>6?h`<button class="t-more" @click=${()=>this.setTab(`audience`)}>All ${e.countries.length} →</button>`:v}</header>
          <div class="t-map-row">
            ${this.renderMap(e.countries)}
            ${this.ranked(e.countries,{dim:`country`,unit:`visitors`,flag:!0,limit:6})}
          </div>
        </section>
        ${this.card(`Devices`,e.devices,{dim:`device`,unit:`visitors`},`audience`)}
        ${t&&!this.filtered?h`<section class="t-card" aria-label="Right now">
              <header><h3>Right now</h3><span class="t-hint">${X(t.visitors)} in the last 5 min · views per minute, last 30 min</span></header>
              ${t.series.some(e=>e.views>0)?h`<lit-material-sparkline label="Page views per minute, last 30 minutes" .values=${t.series.map(e=>e.views)}></lit-material-sparkline>
                    ${this.ranked(t.pages,{dim:`page`,limit:5,unit:`views`})}`:h`<p class="t-none">Nobody has opened a page in the last 30 minutes.</p>`}
            </section>`:v}
      </div>`}renderTiers(){let e=this.data,t=e.tiers??{verified:0,page_only:0,hosting:0},n=e.totals.people.visitors,r=Math.max(0,n-t.verified),i=e.totals.owner??{pages:0,visitors:0},a=[...e.script?[[`Verified`,t.verified,`Ran the page script: a real browser showing the page`]]:[],[e.script?`Likely`:`People`,e.script?r:n,`Loaded the page's styles or scripts, as a browser does`],[`Page only`,t.page_only,`A browser User-Agent that loaded no asset: counted as a bot`],[`Server networks`,t.hosting,`A browser User-Agent from a cloud or hosting network: counted as a bot`]];return h`<div class="t-tiers" role="group" aria-label="How sure">
      ${a.map(([e,t,n])=>h`<span class="t-tier" title=${n}><b>${X(t)}</b> ${e}</span>`)}
      <button class="t-more" aria-expanded=${this.tiersOpen} @click=${()=>this.tiersOpen=!this.tiersOpen}>${this.tiersOpen?`Hide`:`What do these mean?`}</button>
      <span class="t-spacer"></span>
      <span class="t-tier t-owner" title="Your own visits (admin sign-in, owner networks, or this browser's owner link) aren't in these numbers">
        ${i.pages?h`<b>${X(i.pages)}</b> of your page views not counted`:`Your visits aren't counted`}
        <button class="t-more" @click=${()=>this.showOwner()}>${this.owner?`Hide`:`Exclude a browser…`}</button>
      </span>
    </div>
    ${this.tiersOpen?h`<dl class="t-explain">${a.map(([e,,t])=>h`<dt>${e}</dt><dd>${t}.</dd>`)}
          <dt>Not counted</dt><dd>Your own visits: admin sign-in, owner networks, or a browser you excluded.</dd></dl>`:v}
    ${this.owner?this.renderOwner():v}`}renderOwner(){let e=this.owner,t=e.sites.length?e.sites:(this.data?.hosts??[]).filter(e=>/[a-z]/i.test(e)&&e.includes(`.`)&&!e.includes(`:`)&&e!==`other`);return h`<section class="t-card t-wide" aria-label="Exclude a browser">
      <header><h3>Exclude a browser</h3><span class="t-hint">Open the link in each browser and on each phone you use</span></header>
      <p class="t-foot">Signing in here already excludes this browser for the day. A link sets a cookie on that site for a year, so visits from that browser count as yours even without signing in. Keep the links private.${e.networks?` Your [traffic] owner_networks are excluded too.`:``}</p>
      <ul class="t-links">
        ${t.map(t=>{let n=`https://${t}${e.path}`;return h`<li><a href=${n} target="_blank" rel="noopener noreferrer">${t}</a>
            <button class="t-more" @click=${()=>navigator.clipboard?.writeText(n)}>Copy link</button></li>`})}
      </ul>
    </section>`}renderVisitors(){let e=this.data.visits??[];if(!e.length)return h`<lit-material-empty-state headline="No visits yet">Visits appear here when they end (after 30 quiet minutes), and while they're open.</lit-material-empty-state>`;let t=!this.host&&(this.data.hosts?.length??0)>1,n=this.visitFilter.trim().toLowerCase(),r=n?e.filter(e=>[e.country&&Li(e.country),e.host,e.channel,e.referrer,e.browser,e.os,e.device,e.campaign,...e.pages].filter(Boolean).join(` `).toLowerCase().includes(n)):e,i=this.shown.visits??Ni;return h`<label class="t-search"><span class="t-sr">Filter visits</span>
      <input type="search" placeholder="Filter by page, country, source, browser…" .value=${this.visitFilter} @input=${e=>{this.visitFilter=e.target.value,this.shown={...this.shown,visits:Ni}}}></label>
    ${r.length===0?h`<p class="t-none">No visit matches “${this.visitFilter}”.</p>`:v}
    <ol class="t-visits" aria-label="Recent visits">
      ${r.slice(0,i).map(e=>{let n=e.channel===`direct`||!e.channel?`Direct`:`${Ci[e.channel]??e.channel}${e.referrer?` · ${e.referrer}`:``}`,r=[Oi[e.device]??e.device,e.os,e.browser].filter(Boolean).join(` · `),i=e.views-e.pages.length;return h`<li class="t-visit">
          <header>
            <span class="t-visit-who">${e.country?h`<span class="t-code">${e.country}</span>${Li(e.country)}`:`Unknown country`}</span>
            ${e.verified?h`<lit-material-status status="ok">Verified</lit-material-status>`:v}
            ${e.number>1?h`<lit-material-status status="info">Visit ${e.number} today</lit-material-status>`:v}
            <span class="t-spacer"></span>
            <span class="t-hint">${Di(e.end)} · ${e.views===1?`one page`:`${e.views} pages in ${Pi(e.end-e.start)}`}</span>
          </header>
          <p class="t-visit-meta">${t?h`<b>${e.host}</b> · `:v}${n}${e.campaign?` · campaign ${e.campaign}`:``} · ${r||`Unknown device`}</p>
          <ol class="t-path">${e.pages.map(e=>h`<li>${e}</li>`)}${i>0?h`<li>+${i} more</li>`:v}</ol>
          ${e.events.length?h`<p class="t-visit-meta">Events: ${e.events.join(`, `)}</p>`:v}
        </li>`})}
    </ol>
    ${r.length>i?h`<button class="t-more t-show" @click=${()=>this.shown={...this.shown,visits:i+Ni}}>Show ${Math.min(Ni,r.length-i)} more of ${r.length-i}</button>`:v}
    <p class="t-foot">The last ${e.length} visits kept (up to 300 across days). No addresses or User-Agents are kept; a person who comes back another day is a new visitor.</p>`}renderPages(){let e=this.lists();return h`<div class="t-cards">
      ${this.card(`Top pages`,e.pages,{dim:`page`,unit:`views`})}
      ${e.entry.length?this.card(`Entry pages`,e.entry,{dim:`page`,unit:`sessions`,hint:`Where sessions began`}):v}
      ${e.exit.length?this.card(`Exit pages`,e.exit,{dim:`page`,unit:`sessions`,hint:`Where sessions ended`}):v}
      ${e.not_found.length?this.card(`Not found`,e.not_found,{unit:`requests`,hint:`Paths answered 404`,pages:!0}):v}
      ${this.query?v:this.renderLandings()}
      ${this.query?v:this.renderDownloads()}
    </div>`}renderLandings(){let e=(this.data.entries??[]).map(([e,t])=>{let[n,r]=Ei(e);return[`${this.pageName(n)} · ${Ci[r]??r}`,t]});return e.length?this.card(`Landing pages by source`,e,{unit:`visits`,hint:`First page of visits from outside`}):v}renderDownloads(e=null){let t=(this.data.downloads??[]).map(([e,t])=>{let[n,r]=Ei(e);return[n,r,t]}).filter(([,t])=>!e||e.includes(t)).map(([e,t,n])=>[`${this.pageName(e)} · ${Ti[t]??t}`,n]);return t.length?this.card(`Downloads`,t,{unit:`downloads`,hint:`Installers, packages, install scripts`}):v}renderSources(){let e=this.lists();return h`<div class="t-cards">
      ${this.card(`Channels`,e.channels,{dim:`channel`,unit:`visits`,hint:`How sessions arrived`})}
      ${this.card(`Referring sites`,e.referrers,{unit:`visits`})}
      ${e.utm_source.length||e.utm_campaign.length?h`${this.card(`Campaign sources`,e.utm_source,{dim:`utm_source`,unit:`visits`,hint:`utm_source`})}
            ${e.utm_medium.length?this.card(`Campaign media`,e.utm_medium,{unit:`visits`,hint:`utm_medium`}):v}
            ${this.card(`Campaigns`,e.utm_campaign,{dim:`utm_campaign`,unit:`visits`,hint:`utm_campaign`})}`:v}
    </div>`}renderAudience(){let e=this.lists(),t=this.data,n=e=>{let t=e.reduce((e,[,t])=>e+t,0)||1;return e=>`${X(e[1])} · ${Fi(e[1]/t)}`};return h`<section class="t-card t-wide t-map-big" aria-label="Map">${this.renderMap(e.countries)}</section>
    <div class="t-cards">
      ${this.card(`Countries`,e.countries,{dim:`country`,unit:`visitors`,flag:!0,value:n(e.countries)})}
      ${this.card(`Devices`,e.devices,{dim:`device`,unit:`visitors`,value:n(e.devices)})}
      ${this.card(`Browsers`,e.browsers,{dim:`browser`,unit:`visitors`,value:n(e.browsers)})}
      ${this.card(`Operating systems`,e.os,{dim:`os`,unit:`visitors`,value:n(e.os)})}
    </div>
    <p class="t-foot">${t.geo_source?h`Countries: <a href="https://db-ip.com" target="_blank" rel="noopener">IP Geolocation by DB-IP</a> (${t.geo_source}, CC BY 4.0), looked up on this server. No addresses are kept.`:`No country table is loaded (built without the geo feature).`}</p>`}renderBehaviour(){let e=this.data;if(![`time_on_page`,`scroll`,`screens`,`events`,`goals`].some(t=>(e[t]??[]).length))return h`<lit-material-empty-state headline=${e.script?`No reports yet`:`Turn on the page script`}>
        ${e.script?h`Add <code>&lt;script defer src="/_rws/t.js"&gt;&lt;/script&gt;</code> to your pages; time on page, scroll depth, screens, events and goals appear here.`:h`Set <code>[traffic] script = true</code> and add <code>&lt;script defer src="/_rws/t.js"&gt;&lt;/script&gt;</code> to your pages. It reports to this server only, with no cookies.`}
      </lit-material-empty-state>`;let t=e.sessions?.sessions??0,n=[`25`,`50`,`75`,`100`].map(t=>[`up to ${t}%`,((e.scroll??[]).find(([e])=>e===t)??[t,0])[1]]);n.every(([,e])=>!e)&&(n=[]);let r=(e.event_props??[]).map(([e,t])=>[e.replace(`	`,` · `),t]);return h`<div class="t-cards">
      ${this.card(`Time on page`,(e.time_on_page??[]).map(([e,t,n])=>[e,t,n]),{dim:`page`,unit:`seconds`,value:e=>`${Pi(e[1])} · ${X(e[2])}×`,hint:`Average, views reported`})}
      ${this.card(`Goals`,e.goals??[],{unit:`conversions`,value:e=>t?`${X(e[1])} · ${(e[1]/t*100).toFixed(1)}%`:X(e[1]),hint:`Sessions that reached it`})}
      ${this.card(`Events`,e.events??[],{unit:`events`,hint:`rws.track()`})}
      ${r.length?this.card(`Event properties`,r,{unit:`events`}):v}
      ${this.card(`Scroll depth`,n,{unit:`views`})}
      ${this.card(`Screen sizes`,(e.screens??[]).map(([e,t])=>[ki[e]??e,t]),{unit:`views`})}
    </div>`}groupKpis(e){let t=this.data,n=t.totals[e],r=t.previous?.[e],i=r&&(r.requests>0||r.visitors>0);return h`<div class="t-kpis" role="group" aria-label="Key numbers">
      ${[[`requests`,`Requests`,n.requests,i?r.requests:null,`Everything they fetched`],[`visitors`,e===`ai`?`Agents`:e===`apps`?`Users`:`Clients`,n.visitors,i?r.visitors:null,`Distinct clients, counted once a day`],[`fetches`,`Pages`,n.pages,i?r.pages:null,`HTML pages they fetched`]].map(([e,t,n,r,i])=>{let a=Ri(n,r);return h`<button class="t-kpi" aria-pressed=${e===`requests`} ?disabled=${e!==`requests`} title=${i}>
          <span class="t-kpi-label">${t}</span><span class="t-kpi-value">${X(n)}</span>
          <span class="t-kpi-delta" data-tone=${a?`flat`:`none`}>${this.compare&&a?a.text:` `}</span>
        </button>`})}
    </div>`}groupChart(e){let t=this.data,n=t.bucket===`hour`,r=e.map(e=>({name:bi[e],points:t.series.map(t=>({x:t.t*1e3,y:t[e]}))}));if(e.length===1&&this.compare){let n=(t.previous_series??[]).map(t=>({x:t.t*1e3,y:t[e[0]]}));n.some(e=>e.y>0)&&r.push({name:`Period before`,points:n})}return h`<section class="t-chart${e.length>1?` t-chart-multi`:``}" aria-label="Chart">
      <lit-material-line-chart label=${`Requests per ${n?`hour`:`day`}`} unit="requests" height="240" .series=${r}
        .formatX=${n?ji:Mi}></lit-material-line-chart>
    </section>`}renderApps(){let e=this.data,t=e.apps??{users:[],versions:[],events:[]},n=(e,t=e=>e)=>e.map(([e,n])=>{let[r,i]=Ei(e);return[`${r} · ${t(i)}`,n]});return!e.totals.apps?.requests&&!t.users.length?h`${this.groupKpis(`apps`)}
        <lit-material-empty-state headline="No app traffic yet">
          Native apps and browser extensions that call this server are counted here, not as bots. Name yours with
          <code>[traffic] apps = ["MyApp=MyApp/", "MyApp=chrome-extension://&lt;id&gt;"]</code>, and have them send events to
          <code>POST /_rws/e</code> with <code>{"app":"MyApp","v":"1.2","p":"ios","n":"opened"}</code>.
        </lit-material-empty-state>
        <div class="t-cards">${this.renderDownloads()}</div>`:h`
      ${this.groupKpis(`apps`)}
      ${this.groupChart([`apps`])}
      <div class="t-cards">
        ${this.card(`Users by app and platform`,n(t.users,e=>wi[e]??e),{unit:`users`,hint:`Clients counted once a day`})}
        ${this.card(`Versions`,n(t.versions),{unit:`users`})}
        ${this.card(`App events`,n(t.events),{unit:`events`,hint:`POST /_rws/e`})}
        ${this.renderDownloads()}
      </div>
      <p class="t-foot">Apps are recognised by <code>[traffic] apps</code> (a User-Agent token or an extension origin), or by platform: Apple's and Android's networking stacks, Electron, extension origins. Unnamed ones are grouped by platform.</p>`}renderRobots(e){let t=this.data,n=e=>{let t=new Map;for(let[n,r]of e)t.set(n,(t.get(n)??0)+r);return[...t.entries()].sort((e,t)=>t[1]-e[1])};if(e===`ai`){let e=t.ai_agents.map(e=>[`${e.name}${e.operator?` (${e.operator})`:``}`,e.requests,e]),r=n(t.ai_agents.map(e=>[{"ai-training":`Training crawlers`,"ai-search":`AI search`,"ai-assistant":`Assistants (for a person)`,"ai-agent":`Agents`}[e.kind]??e.kind,e.requests]));return h`
        ${this.groupKpis(`ai`)}
        ${this.groupChart([`ai`])}
        <div class="t-cards">
          ${this.card(`AI agents`,e,{unit:`requests`,value:e=>h`${X(e[1])}${e[2].verified?h` <lit-material-status status="ok">Verified</lit-material-status>`:v}`,hint:`Verified: from the operator's published network`})}
          ${this.card(`Pages they read`,t.ai_pages,{unit:`requests`,pages:!0})}
          ${this.card(`What for`,r,{unit:`requests`})}
        </div>`}let r=e=>{let t=e.kind.replaceAll(`-`,` `);return e.name&&e.name!==t.replace(`browser no assets`,`browser, no assets`)?`${t} · ${e.name}`:t},i=n(t.bots.map(e=>[e.kind.replaceAll(`-`,` `),e.requests]));return h`
      ${this.groupKpis(`bots`)}
      ${this.groupChart([`bots`])}
      <div class="t-cards">
        ${this.card(`By kind`,i,{unit:`requests`,hint:`Search, SEO, monitors, scanners…`})}
        ${this.card(`Bots`,t.bots.map(e=>[r(e),e.requests]),{unit:`requests`})}
        ${t.not_found.length?this.card(`Not found`,t.not_found,{unit:`requests`,hint:`404s, everyone (scanners mostly)`,pages:!0}):v}
        ${(t.crawl??[]).length?this.card(`Crawlers`,t.crawl.map(e=>[e.name,e.pages+e.not_found,e]),{unit:`requests`,value:e=>`${X(e[2].pages)} pages${e[2].not_found?` · ${X(e[2].not_found)} 404`:``}`,hint:`Named search, AI and other crawlers`}):v}
        ${(t.crawl_404??[]).length?this.card(`404s crawlers hit`,t.crawl_404.map(([e,t])=>{let[n,r]=Ei(e);return[`${this.pageName(n)} · ${r}`,t]}),{unit:`requests`,hint:`Fix or redirect these`}):v}
        ${(t.probes?.paths??[]).length?this.card(`Probes`,t.probes.paths,{unit:`requests`,hint:`Scanners looking for secrets and admin pages`}):v}
        ${(t.probes?.sources??[]).length?this.card(`Probe sources`,t.probes.sources.map(([e,t])=>{let[n,r,i]=Ei(e);return[[Li(n),r===`-`?``:r,i??``].filter(Boolean).join(` · `),t]}),{unit:`requests`,hint:`Country and network`}):v}
        ${this.renderDownloads([`bots`])}
      </div>
      <p class="t-foot">People are browsers that load a page's styles or scripts; a browser that never does is counted here as "browser, no assets". Spoofed: claims to be a known crawler from outside its network.</p>`}renderAll(){let e=this.data,t=[`people`,`apps`,`ai`,`bots`].map(t=>{let n=e.totals[t],r=e.previous?.[t],i=Ri(n.requests,r&&r.requests?r.requests:null);return h`<button class="t-kpi" @click=${()=>this.setGroup(t)} title=${`Open ${bi[t]}`}>
        <span class="t-kpi-label">${bi[t]}</span><span class="t-kpi-value">${X(n.visitors)}</span>
        <span class="t-kpi-delta">${X(n.requests)} requests${this.compare&&i?` · ${i.text}`:``}</span>
      </button>`}),n=[`people`,`apps`,`ai`,`bots`].reduce((t,n)=>t+(e.totals[n]?.requests??0),0)||1,r=[`people`,`apps`,`ai`,`bots`].map(t=>[bi[t],e.totals[t]?.requests??0]);return h`
      <div class="t-kpis" role="group" aria-label="Groups">${t}</div>
      ${this.groupChart([`people`,`apps`,`ai`,`bots`])}
      <div class="t-cards">
        ${this.card(`Share of requests`,r,{unit:`requests`,value:e=>`${X(e[1])} · ${Fi(e[1]/n)}`})}
        ${this.card(`Top pages (humans)`,e.people_pages,{dim:`page`,unit:`views`,limit:6})}
        ${this.card(`AI agents`,e.ai_agents.map(e=>[e.name,e.requests]),{unit:`requests`,limit:6})}
      </div>`}renderMap(e){return h`<lit-material-world-map
      class="t-map"
      label="Visitors by country"
      unit="visitors"
      selectable
      .data=${Object.fromEntries(e.filter(([e])=>e.length===2))}
      .selected=${Xr(this.filters.country??``)}
      .format=${X}
      @change=${e=>e.detail.code?this.pick(`country`,e.detail.code):this.unpick(`country`)}
    ></lit-material-world-map>`}tabKeys(e){let t=J.findIndex(([e])=>e===this.tab),n={ArrowRight:t+1,ArrowLeft:t-1,Home:0,End:J.length-1}[e.key];if(n===void 0)return;e.preventDefault();let r=(n+J.length)%J.length;this.setTab(J[r][0]),this.updateComplete.then(()=>{let e=this.querySelectorAll(`.t-tabs [role="tab"]`)[r];e?.focus(),e?.scrollIntoView({block:`nearest`,inline:`nearest`})})}render(){if(this.off)return h`<h1>Traffic</h1><lit-material-empty-state headline="Traffic counting is off">It's on whenever the admin API is, unless <code>[traffic] enabled = false</code>.</lit-material-empty-state>`;let e=this.data,t=!e&&this.failed?h`<lit-material-empty-state headline="Traffic couldn't be loaded">${this.failed}
          <lit-material-button slot="actions" variant="tonal" @click=${()=>this.reload()}>Retry</lit-material-button></lit-material-empty-state>`:e?this.query===null&&this.filtered&&!this.notice?h`<p class="t-none">Loading…</p>`:this.group===`people`?({overview:()=>this.renderOverview(),visitors:()=>this.renderVisitors(),pages:()=>this.renderPages(),sources:()=>this.renderSources(),audience:()=>this.renderAudience(),behaviour:()=>this.renderBehaviour()}[this.tab]??(()=>this.renderOverview()))():this.group===`all`?this.renderAll():this.group===`apps`?this.renderApps():this.renderRobots(this.group):h`<div class="t-loading" aria-busy="true" aria-label="Loading">
          <lit-material-skeleton variant="rounded" height="88px"></lit-material-skeleton>
          <lit-material-skeleton variant="rounded" height="240px"></lit-material-skeleton>
          <div class="t-cards">${[0,1].map(()=>h`<lit-material-skeleton variant="rounded" height="200px"></lit-material-skeleton>`)}</div></div>`;return h`
      <div class="t-head">
        <h1>Traffic</h1>
        ${this.renderBar()}
        ${this.group===`people`?h`<div class="t-tabs" role="tablist" aria-label="Traffic sections" @keydown=${e=>this.tabKeys(e)}>
              ${J.map(([e,t])=>h`<button role="tab" aria-selected=${this.tab===e} tabindex=${this.tab===e?0:-1} @click=${t=>{this.setTab(e),t.target.scrollIntoView({block:`nearest`,inline:`nearest`})}}>${t}</button>`)}
            </div>`:h`<div class="t-tabs t-tabs-empty"></div>`}
      </div>
      ${e&&this.failed?h`<p class="t-notice" role="alert">Couldn't refresh: ${this.failed} <button class="t-more" @click=${()=>this.reload()}>Retry</button></p>`:v}
      <div role="tabpanel" class=${this.loading&&e?`t-stale`:``} aria-busy=${this.loading} aria-label=${this.group===`people`?(J.find(([e])=>e===this.tab)??J[0])[1]:bi[this.group]??`All`}>${t}</div>`}};customElements.define(`rws-traffic`,zi);var Bi=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},Vi=class extends y{constructor(){super(...arguments),this.value=``,this.position=`end`,this.tailLength=8,this.truncated=!1,this.ownTitle=null}static{this.styles=o`
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
  `}connectedCallback(){super.connectedCallback(),typeof ResizeObserver<`u`&&(this.resizeObserver=new ResizeObserver(()=>this.measure()),this.resizeObserver.observe(this))}disconnectedCallback(){super.disconnectedCallback(),this.resizeObserver?.disconnect()}updated(){this.resizeObserver&&this.measure()}measure(){let e=this.renderRoot.querySelector(`.start`),t=!!e&&e.scrollWidth>e.clientWidth;t!==this.truncated&&(this.truncated=t);let n=this.getAttribute(`title`);(n===null||n===this.ownTitle)&&(t&&n!==this.value?(this.setAttribute(`title`,this.value),this.ownTitle=this.value):!t&&n!==null&&(this.removeAttribute(`title`),this.ownTitle=null))}render(){let e=[...this.value],t=this.position===`middle`&&e.length>this.tailLength&&this.tailLength>0,n=t?e.slice(0,-this.tailLength).join(``):this.value,r=t?e.slice(-this.tailLength).join(``):``;return h`<span part="start" class="start">${n}</span>${t?h`<span part="tail" class="tail">${r}</span>`:v}`}};Bi([x()],Vi.prototype,`value`,void 0),Bi([x({reflect:!0})],Vi.prototype,`position`,void 0),Bi([x({type:Number,attribute:`tail-length`})],Vi.prototype,`tailLength`,void 0),Bi([x({type:Boolean,reflect:!0})],Vi.prototype,`truncated`,void 0),Vi=Bi([b(`lit-material-truncate`)],Vi);var Hi=class extends nn{constructor(e){if(super(e),this.it=v,e.type!==E.CHILD)throw Error(this.constructor.directiveName+`() can only be used in child bindings`)}render(e){if(e===v||e==null)return this._t=void 0,this.it=e;if(e===_)return e;if(typeof e!=`string`)throw Error(this.constructor.directiveName+`() called with a non-string value`);if(e===this.it)return this._t;this.it=e;let t=[e];return t.raw=t,this._t={_$litType$:this.constructor.resultType,strings:t,values:[]}}};Hi.directiveName=`unsafeHTML`,Hi.resultType=1;var Ui=class extends Hi{};Ui.directiveName=`unsafeSVG`,Ui.resultType=2;var Wi=tn(Ui),Gi=new Map,Ki=new EventTarget,qi=new Set,Ji=`icons-changed`,Yi=Ki;function Xi(e){let t=e.replace(/^\s*(<\?xml[^>]*>\s*)?(<!--[\s\S]*?-->\s*)*/,``).trim(),n=/^<svg\b[^>]*>/i.exec(t);if(!n)return``;let r=n[0].replace(/\s(width|height|aria-hidden|focusable)\s*=\s*("[^"]*"|'[^']*')/gi,``).replace(/\s*\/?>$/,``);return/\sfill\s*=/i.test(r)||(r+=` fill="currentColor"`),r+=` aria-hidden="true" focusable="false">`,r+t.slice(n[0].length)}function Zi(e,t={}){for(let[n,r]of Object.entries(e)){let e=t.prefix?`${t.prefix}:${n}`:n,i=Xi(r);i?(Gi.set(e,i),qi.delete(e)):console.warn(`lit-material-icon: "${e}" is not an <svg>; skipped.`)}Ki.dispatchEvent(new Event(Ji))}function Qi(e){return Gi.get(e)}function $i(e){qi.has(e)||(qi.add(e),console.warn(`lit-material-icon: no icon registered as "${e}".`))}var ea=o`
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

  :host([hidden]) {
    display: none;
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
`,ta=function(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a},na=class extends y{constructor(){super(...arguments),this.size=`medium`,this.color=`inherit`,this.label=``,this.name=``,this.onIconsChanged=()=>{this.name&&this.requestUpdate()}}static{this.styles=ea}connectedCallback(){super.connectedCallback(),Yi.addEventListener(Ji,this.onIconsChanged)}disconnectedCallback(){super.disconnectedCallback(),Yi.removeEventListener(Ji,this.onIconsChanged)}willUpdate(){this.label?(this.setAttribute(`role`,`img`),this.setAttribute(`aria-label`,this.label),this.removeAttribute(`aria-hidden`)):(this.removeAttribute(`role`),this.removeAttribute(`aria-label`),this.setAttribute(`aria-hidden`,`true`))}render(){if(!this.name)return h`<slot part="icon"></slot>`;let e=Qi(this.name);if(e===void 0){let e=this.name;return queueMicrotask(()=>Qi(e)===void 0&&$i(e)),h``}return Wi(e)}};ta([x({reflect:!0})],na.prototype,`size`,void 0),ta([x({reflect:!0})],na.prototype,`color`,void 0),ta([x()],na.prototype,`label`,void 0),ta([x({reflect:!0})],na.prototype,`name`,void 0),na=ta([b(`lit-material-icon`)],na);function Z(e,t={}){return h`<lit-material-icon
    name=${e}
    label=${t.label??v}
    size=${t.size??v}
    slot=${t.slot??v}
  ></lit-material-icon>`}Zi({gauge:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M207.06,72.67A111.24,111.24,0,0,0,128,40h-.4C66.07,40.21,16,91,16,153.13V176a16,16,0,0,0,16,16H224a16,16,0,0,0,16-16V152A111.25,111.25,0,0,0,207.06,72.67ZM224,176H119.71l54.76-75.3a8,8,0,0,0-12.94-9.42L99.92,176H32V153.13c0-3.08.15-6.12.43-9.13H56a8,8,0,0,0,0-16H35.27c10.32-38.86,44-68.24,84.73-71.66V80a8,8,0,0,0,16,0V56.33A96.14,96.14,0,0,1,221,128H200a8,8,0,0,0,0,16h23.67c.21,2.65.33,5.31.33,8Z"/></svg>`,"chart-line":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M232,208a8,8,0,0,1-8,8H32a8,8,0,0,1-8-8V48a8,8,0,0,1,16,0v94.37L90.73,98a8,8,0,0,1,10.07-.38l58.81,44.11L218.73,90a8,8,0,1,1,10.54,12l-64,56a8,8,0,0,1-10.07.38L96.39,114.29,40,163.63V200H224A8,8,0,0,1,232,208Z"/></svg>`,"tree-structure":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M160,112h48a16,16,0,0,0,16-16V48a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16V64H128a24,24,0,0,0-24,24v32H72v-8A16,16,0,0,0,56,96H24A16,16,0,0,0,8,112v32a16,16,0,0,0,16,16H56a16,16,0,0,0,16-16v-8h32v32a24,24,0,0,0,24,24h16v16a16,16,0,0,0,16,16h48a16,16,0,0,0,16-16V160a16,16,0,0,0-16-16H160a16,16,0,0,0-16,16v16H128a8,8,0,0,1-8-8V88a8,8,0,0,1,8-8h16V96A16,16,0,0,0,160,112ZM56,144H24V112H56v32Zm104,16h48v48H160Zm0-112h48V96H160Z"/></svg>`,certificate:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,136a8,8,0,0,1-8,8H72a8,8,0,0,1,0-16h48A8,8,0,0,1,128,136Zm-8-40H72a8,8,0,0,0,0,16h48a8,8,0,0,0,0-16Zm112,65.47V224A8,8,0,0,1,220,231l-24-13.74L172,231A8,8,0,0,1,160,224V200H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216a16,16,0,0,1,16,16V86.53a51.88,51.88,0,0,1,0,74.94ZM160,184V161.47A52,52,0,0,1,216,76V56H40V184Zm56-12a51.88,51.88,0,0,1-40,0v38.22l16-9.16a8,8,0,0,1,7.94,0l16,9.16Zm16-48a36,36,0,1,0-36,36A36,36,0,0,0,232,124Z"/></svg>`,"sliders-horizontal":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M40,88H73a32,32,0,0,0,62,0h81a8,8,0,0,0,0-16H135a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16Zm64-24A16,16,0,1,1,88,80,16,16,0,0,1,104,64ZM216,168H199a32,32,0,0,0-62,0H40a8,8,0,0,0,0,16h97a32,32,0,0,0,62,0h17a8,8,0,0,0,0-16Zm-48,24a16,16,0,1,1,16-16A16,16,0,0,1,168,192Z"/></svg>`,"list-bullets":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M80,64a8,8,0,0,1,8-8H216a8,8,0,0,1,0,16H88A8,8,0,0,1,80,64Zm136,56H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16Zm0,64H88a8,8,0,0,0,0,16H216a8,8,0,0,0,0-16ZM44,52A12,12,0,1,0,56,64,12,12,0,0,0,44,52Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,116Zm0,64a12,12,0,1,0,12,12A12,12,0,0,0,44,180Z"/></svg>`,"terminal-window":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,128a8,8,0,0,1-3,6.25l-40,32a8,8,0,1,1-10-12.5L107.19,128,75,102.25a8,8,0,1,1,10-12.5l40,32A8,8,0,0,1,128,128Zm48,24H136a8,8,0,0,0,0,16h40a8,8,0,0,0,0-16Zm56-96V200a16,16,0,0,1-16,16H40a16,16,0,0,1-16-16V56A16,16,0,0,1,40,40H216A16,16,0,0,1,232,56ZM216,200V56H40V200H216Z"/></svg>`,"sign-out":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M120,216a8,8,0,0,1-8,8H48a8,8,0,0,1-8-8V40a8,8,0,0,1,8-8h64a8,8,0,0,1,0,16H56V208h56A8,8,0,0,1,120,216Zm109.66-93.66-40-40a8,8,0,0,0-11.32,11.32L204.69,120H112a8,8,0,0,0,0,16h92.69l-26.35,26.34a8,8,0,0,0,11.32,11.32l40-40A8,8,0,0,0,229.66,122.34Z"/></svg>`,"check-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M173.66,98.34a8,8,0,0,1,0,11.32l-56,56a8,8,0,0,1-11.32,0l-24-24a8,8,0,0,1,11.32-11.32L112,148.69l50.34-50.35A8,8,0,0,1,173.66,98.34ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,"warning-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24Zm0,192a88,88,0,1,1,88-88A88.1,88.1,0,0,1,128,216Zm-8-80V80a8,8,0,0,1,16,0v56a8,8,0,0,1-16,0Zm20,36a12,12,0,1,1-12-12A12,12,0,0,1,140,172Z"/></svg>`,"x-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M165.66,101.66,139.31,128l26.35,26.34a8,8,0,0,1-11.32,11.32L128,139.31l-26.34,26.35a8,8,0,0,1-11.32-11.32L116.69,128,90.34,101.66a8,8,0,0,1,11.32-11.32L128,116.69l26.34-26.35a8,8,0,0,1,11.32,11.32ZM232,128A104,104,0,1,1,128,24,104.11,104.11,0,0,1,232,128Zm-16,0a88,88,0,1,0-88,88A88.1,88.1,0,0,0,216,128Z"/></svg>`,pause:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M200,32H160a16,16,0,0,0-16,16V208a16,16,0,0,0,16,16h40a16,16,0,0,0,16-16V48A16,16,0,0,0,200,32Zm0,176H160V48h40ZM96,32H56A16,16,0,0,0,40,48V208a16,16,0,0,0,16,16H96a16,16,0,0,0,16-16V48A16,16,0,0,0,96,32Zm0,176H56V48H96Z"/></svg>`,play:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M232.4,114.49,88.32,26.35a16,16,0,0,0-16.2-.3A15.86,15.86,0,0,0,64,39.87V216.13A15.94,15.94,0,0,0,80,232a16.07,16.07,0,0,0,8.36-2.35L232.4,141.51a15.81,15.81,0,0,0,0-27ZM80,215.94V40l143.83,88Z"/></svg>`,"arrow-clockwise":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M240,56v48a8,8,0,0,1-8,8H184a8,8,0,0,1,0-16H211.4L184.81,71.64l-.25-.24a80,80,0,1,0-1.67,114.78,8,8,0,0,1,11,11.63A95.44,95.44,0,0,1,128,224h-1.32A96,96,0,1,1,195.75,60L224,85.8V56a8,8,0,1,1,16,0Z"/></svg>`,trash:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M216,48H176V40a24,24,0,0,0-24-24H104A24,24,0,0,0,80,40v8H40a8,8,0,0,0,0,16h8V208a16,16,0,0,0,16,16H192a16,16,0,0,0,16-16V64h8a8,8,0,0,0,0-16ZM96,40a8,8,0,0,1,8-8h48a8,8,0,0,1,8,8v8H96Zm96,168H64V64H192ZM112,104v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Zm48,0v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Z"/></svg>`,plus:`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M224,128a8,8,0,0,1-8,8H136v80a8,8,0,0,1-16,0V136H40a8,8,0,0,1,0-16h80V40a8,8,0,0,1,16,0v80h80A8,8,0,0,1,224,128Z"/></svg>`,"lock-key":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,112a28,28,0,0,0-8,54.83V184a8,8,0,0,0,16,0V166.83A28,28,0,0,0,128,112Zm0,40a12,12,0,1,1,12-12A12,12,0,0,1,128,152Zm80-72H176V56a48,48,0,0,0-96,0V80H48A16,16,0,0,0,32,96V208a16,16,0,0,0,16,16H208a16,16,0,0,0,16-16V96A16,16,0,0,0,208,80ZM96,56a32,32,0,0,1,64,0V80H96ZM208,208H48V96H208V208Z"/></svg>`,"clipboard-text":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M168,152a8,8,0,0,1-8,8H96a8,8,0,0,1,0-16h64A8,8,0,0,1,168,152Zm-8-40H96a8,8,0,0,0,0,16h64a8,8,0,0,0,0-16Zm56-64V216a16,16,0,0,1-16,16H56a16,16,0,0,1-16-16V48A16,16,0,0,1,56,32H92.26a47.92,47.92,0,0,1,71.48,0H200A16,16,0,0,1,216,48ZM96,64h64a32,32,0,0,0-64,0ZM200,48H173.25A47.93,47.93,0,0,1,176,64v8a8,8,0,0,1-8,8H88a8,8,0,0,1-8-8V64a47.93,47.93,0,0,1,2.75-16H56V216H200Z"/></svg>`,"user-circle":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24ZM74.08,197.5a64,64,0,0,1,107.84,0,87.83,87.83,0,0,1-107.84,0ZM96,120a32,32,0,1,1,32,32A32,32,0,0,1,96,120Zm97.76,66.41a79.66,79.66,0,0,0-36.06-28.75,48,48,0,1,0-59.4,0,79.66,79.66,0,0,0-36.06,28.75,88,88,0,1,1,131.52,0Z"/></svg>`,"dots-three":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M140,128a12,12,0,1,1-12-12A12,12,0,0,1,140,128Zm56-12a12,12,0,1,0,12,12A12,12,0,0,0,196,116ZM60,116a12,12,0,1,0,12,12A12,12,0,0,0,60,116Z"/></svg>`,"magnifying-glass":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M229.66,218.34l-50.07-50.06a88.11,88.11,0,1,0-11.31,11.31l50.06,50.07a8,8,0,0,0,11.32-11.32ZM40,112a72,72,0,1,1,72,72A72.08,72.08,0,0,1,40,112Z"/></svg>`,"download-simple":`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M224,144v64a8,8,0,0,1-8,8H40a8,8,0,0,1-8-8V144a8,8,0,0,1,16,0v56H208V144a8,8,0,0,1,16,0Zm-101.66,5.66a8,8,0,0,0,11.32,0l40-40a8,8,0,0,0-11.32-11.32L136,124.69V32a8,8,0,0,0-16,0v92.69L93.66,98.34a8,8,0,0,0-11.32,11.32Z"/></svg>`});var ra=[{id:`overview`,label:`Overview`,icon:`gauge`,key:`o`,group:`Observe`},{id:`traffic`,label:`Traffic`,icon:`chart-line`,key:`t`,group:`Observe`},{id:`logs`,label:`Logs`,icon:`terminal-window`,key:`l`,group:`Observe`},{id:`upstreams`,label:`Upstreams`,icon:`tree-structure`,key:`u`,group:`Route`},{id:`certificates`,label:`Certificates`,icon:`certificate`,key:`c`,group:`Route`},{id:`controls`,label:`Controls`,icon:`sliders-horizontal`,key:`x`,group:`Operate`},{id:`config`,label:`Config`,icon:`list-bullets`,key:`f`,group:`Operate`},{id:`audit`,label:`Audit`,icon:`clipboard-text`,key:`a`,group:`Operate`}],ia=[`Observe`,`Route`,`Operate`],aa=[`overview`,`traffic`,`upstreams`,`logs`],oa=1e3,sa=[[`Admin console guide`,`https://rws8.tech/docs/deployment/admin/`],[`Traffic analytics guide`,`https://rws8.tech/docs/deployment/traffic/`],[`Config file reference`,`https://rws8.tech/docs/configuration/config-file/`]],ca=[{keys:[],label:`Changes are written to Audit`}],la=/Mac|iPhone|iPad/.test(navigator.platform??``)?`⌘`:`Ctrl`,ua=e=>/^(\d{1,3}\.){3}\d{1,3}$/.test(e)&&e.split(`.`).every(e=>Number(e)<=255)||/^[0-9a-f:.]+$/i.test(e)&&e.includes(`:`),da=e=>/expected \{/.test(e??``)?`The server didn't accept that value.`:e;function fa(){let e=location.hash.replace(/^#\/?/,``);return ra.some(t=>t.id===e)?e:`overview`}var pa=e=>Gr(e*1e3,{unitDisplay:`narrow`}),ma=h`<svg class="brand-mark" viewBox="0 0 32 32" aria-hidden="true"><rect width="32" height="32" rx="7" fill="#D2551E"/><g transform="translate(4 3.5)"><rect x="2" y="5" width="20" height="4" rx="1.5" fill="#16120F"/><rect x="2" y="10.5" width="20" height="4" rx="1.5" fill="#16120F"/><path d="M3.5 16H15L14 17L15 18L14 19L15 20H3.5A1.5 1.5 0 0 1 2 18.5V17.5A1.5 1.5 0 0 1 3.5 16Z" fill="#F2EBE3"/><rect x="16.5" y="16.5" width="2" height="2" fill="#F2EBE3"/><rect x="19.5" y="15" width="1.4" height="1.4" fill="#F2EBE3"/></g></svg>`,Q=(e,t)=>h`<lit-material-status status=${e}>${t}</lit-material-status>`,ha=e=>new Date(e).toLocaleTimeString([],{hour12:!1}),$=(e,t,n)=>`${e.toLocaleString()} ${e===1?t:n}`,ga=e=>e<10?e.toFixed(1):Math.round(e).toLocaleString();function _a(e,t=Date.now()){let n=Math.max(0,Math.round((t-e)/1e3));return n<2?`just now`:n<60?`${n} s ago`:n<3600?`${Math.floor(n/60)} min ago`:n<86400?`${Math.floor(n/3600)} h ago`:new Date(e).toLocaleDateString()}function va(e){if(e<=1)return 1;let t=10**Math.floor(Math.log10(e)),n=e/t;return(n<=1?1:n<=2?2:n<=5?5:10)*t}function ya(e){try{let t=JSON.parse(e);if(t&&typeof t==`object`&&t.status!==void 0)return{raw:e,j:t}}catch{}return{raw:e,j:null}}var ba=e=>e.j?String(e.j.status)[0]:/"\s([1-5])\d\d\s/.exec(e.raw)?.[1]??``,xa=e=>e>=500?`bad`:e>=400?`warn`:e>=300?`info`:`ok`;function Sa(e){let t=new Date(e);return Number.isNaN(t.getTime())?String(e??``):ha(t.getTime())}function Ca(e){if(!e.j)return h`<div class="req" role="row"><span class="c-raw" role="cell" title=${e.raw}>${e.raw}</span></div>`;let t=e.j,n=t.path??t.uri??``;return h`<div class="req" role="row">
    <span class="c-time" role="cell">${Sa(t.time)}</span>
    <span class="c-method" role="cell">${t.method??``}</span>
    <span class="c-url" role="cell" title="${t.host??``}${n}">${t.host?h`<span class="host">${t.host}</span>`:v}${n}</span>
    <span class="c-status" role="cell" data-tone=${xa(Number(t.status))}>${t.status}</span>
    <span class="c-size" role="cell">${typeof t.bytes==`number`?Ur(t.bytes):``}</span>
    <span class="c-from" role="cell">${t.remote_addr??``}</span>
  </div>`}var wa=h`<div class="req req-head" role="row">
  <span role="columnheader">Time</span><span role="columnheader">Method</span><span role="columnheader">URL</span>
  <span role="columnheader">Status</span><span class="c-size" role="columnheader">Size</span><span class="c-from" role="columnheader">From</span>
</div>`;function Ta(e){let t=e.path.replace(`/admin/api/v1`,``),n=null;try{n=JSON.parse(e.detail)}catch{}return/\/drain$/.test(t)&&n?.backend?`${n.drained?`Drained`:`Put back`} ${n.backend}`:`${e.method===`TOOL`?`tool`:e.method} ${t}`}var Ea=class extends y{static properties={at:{type:Number},now:{state:!0}};createRenderRoot(){return this}connectedCallback(){super.connectedCallback(),this.tick=setInterval(()=>this.now=Date.now(),1e3)}disconnectedCallback(){super.disconnectedCallback(),clearInterval(this.tick)}render(){return this.at?h`updated ${_a(this.at,Math.max(this.now??0,Date.now()))}`:v}};customElements.define(`rws-ago`,Ea);var Da=class extends y{static properties={authed:{state:!0},view:{state:!0},loginError:{state:!0},data:{state:!0},error:{state:!0},samples:{state:!0},recent:{state:!0},hover:{state:!0},paused:{state:!0},logTick:{state:!0},logFilter:{state:!0},filter:{state:!0},busy:{state:!0},me:{state:!0},sso:{state:!0},moreOpen:{state:!0},conn:{state:!0},notice:{state:!0},ask:{state:!0},typed:{state:!0},query:{state:!0},fieldError:{state:!0}};constructor(){super(),this.authed=!1,this.view=fa(),this.data={},this.samples=[],this.recent=null,this.hover=null,this.paused=!1,this.logTick=0,this.logAll=[],this.logFrozen=[],this.logFilter={text:``,status:`all`},this.filter=``,this.busy=``,this.lastTotal=null,this.me=null,this.sso=!1,this.moreOpen=!1,this.conn={lost:!1,at:0},this.notice=null,this.ask=null,this.typed=``,this.query=``,this.fieldError={}}createRenderRoot(){return this}connectedCallback(){super.connectedCallback(),this.onHash=()=>this.show(fa(),{push:!1}),addEventListener(`hashchange`,this.onHash),this.onKey=e=>this.key(e),document.addEventListener(`keydown`,this.onKey),this.check({quiet:!Lr()}),G(`/auth`).then(e=>this.sso=!!e?.oidc).catch(()=>{})}disconnectedCallback(){super.disconnectedCallback(),removeEventListener(`hashchange`,this.onHash),document.removeEventListener(`keydown`,this.onKey),this.stop(),clearInterval(this.pulse)}async check({quiet:e=!1}={}){try{let[e,t]=await Promise.all([G(`/whoami`),G(`/status`)]);this.me=e,this.data={...this.data,status:t},this.authed=!0,this.loginError=``,this.conn={lost:!1,at:Date.now()},this.show(this.view,{push:!1}),this.heartbeat()}catch(t){if(this.authed=!1,e&&t instanceof W&&t.status===401){this.focusToken();return}this.loginError=t instanceof W&&t.status===401?`That token isn't an admin token. Check RWS_ADMIN_TOKEN (or your token_env) on the server.`:t instanceof W&&t.status===403?t.message:`The admin API didn't answer: ${t.message}`,t instanceof W&&t.status===401&&Rr(``),this.focusToken()}}focusToken(){this.updateComplete.then(()=>this.querySelector(`#token`)?.focus())}heartbeat(){clearInterval(this.pulse);let e=0,t=async()=>{if(this.authed)try{let[t,n]=await Promise.all([G(`/status`),G(`/upstreams`).catch(()=>this.data.upstreams??[])]);if(this.data={...this.data,status:t,upstreams:n},this.conn={lost:!1,at:Date.now()},e++%12==0){let e=await G(`/certificates`).catch(()=>null);e&&(this.data={...this.data,certificates:e})}}catch(e){if(e instanceof W&&e.status===401){this.logout(),this.loginError=`The server no longer accepts this token. Sign in again.`;return}this.conn={...this.conn,lost:!0}}};this.pulse=setInterval(t,5e3),t()}login(e){e.preventDefault();let t=(this.querySelector(`#token`)?.value??``).trim();t?(Rr(t),this.check()):this.loginError=`Paste the admin token.`}logout(){let e=this.me?.via===`oidc`;Rr(``),this.stop(),this.authed=!1,this.data={},this.me=null,this.samples=[],this.lastTotal=null,this.recent=null,clearInterval(this.pulse),e&&location.assign(`/admin/auth/logout`)}async call(e,t){try{let n=await G(e,t);return this.error=``,this.conn={lost:!1,at:Date.now()},n}catch(e){throw e instanceof W&&e.status===401?(this.logout(),this.loginError=`The server no longer accepts this token. Sign in again.`):e instanceof W?this.error=da(e.message):this.conn={...this.conn,lost:!0},e}}toast(e,t=null){clearTimeout(this.noticeTimer),this.notice={message:e,undo:t},this.noticeTimer=setTimeout(()=>this.notice=null,t?8e3:4e3)}confirm({title:e,body:t,action:n,tone:r=`danger`,effects:i=null,type:a=``}){return new Promise(o=>{this.typed=``,this.ask={title:e,body:t,action:n,tone:r,effects:i,type:a,resolve:o},this.updateComplete.then(()=>{this.querySelector(`#confirm`)?.showModal(),(this.querySelector(`#confirm-type`)??this.querySelector(`#confirm .cancel`))?.focus()})})}confirmReady(){return!this.ask?.type||this.typed.trim().toLowerCase()===this.ask.type}answer(e){let t=this.ask;this.querySelector(`#confirm`)?.close(),this.ask=null,t?.resolve(e)}key(e){if(!this.authed)return;let t=this.querySelector(`lit-material-command-palette`),n=(e.key??``).toLowerCase();if((e.ctrlKey||e.metaKey)&&!e.altKey&&!e.shiftKey&&n===`k`)e.preventDefault(),t?.open?t.hide():this.openPalette();else if(!(e.ctrlKey||e.metaKey||e.altKey)&&!(t?.open||this.ask||this.moreOpen&&n!==`escape`)&&!e.composedPath().some(e=>e instanceof Element&&e.matches(`input, textarea, select, [contenteditable]`))){if(n===`escape`&&this.moreOpen)this.moreOpen=!1;else if(this.chord&&Date.now()-this.chord<1e3){this.chord=0;let t=ra.find(e=>e.key===n);t&&(e.preventDefault(),this.show(t.id))}else n===`g`?this.chord=Date.now():n===`/`&&(e.preventDefault(),this.openPalette())}}openPalette(){if(!this.ask){this.query=``,this.moreOpen=!1;for(let[e,t]of[[`caches`,`/caches`],[`config`,`/config`],[`certificates`,`/certificates`],[`blocklist`,`/blocklist`]])this.data[e]||G(t).then(t=>this.data={...this.data,[e]:t}).catch(()=>{});this.querySelector(`lit-material-command-palette`)?.show()}}paletteItems(){let e=this.data,t=[],n=(e,n,r,i={})=>t.push({id:`${e}:${n}:${i.hint??``}`,section:e,label:n,run:r,...i}),r={hint:`asks you to confirm`},i=this.query.trim();ua(i)&&((e.blocklist??[]).includes(i)?n(`Actions`,`Unblock ${i}`,()=>this.unblock(i)):n(`Actions`,`Block ${i}`,()=>this.blockIp(i),{hint:`refuses every request from it`})),n(`Actions`,`Reload configuration`,()=>this.reload(),{hint:`re-reads rws.config.toml`,keywords:[`config`,`restart`,`apply`]}),e.status?.maintenance?n(`Actions`,`Turn off maintenance mode`,()=>this.setMaintenance(!1),{keywords:[`503`]}):n(`Actions`,`Turn on maintenance mode`,()=>this.setMaintenance(!0),{...r,keywords:[`503`,`down`]}),e.certificates?.acme?.running&&n(`Actions`,`Renew certificates now`,()=>this.renew(),{keywords:[`acme`,`tls`,`https`]});for(let t of e.upstreams??[])for(let e of t.backends)e.drained?n(`Actions`,`Put back ${e.url}`,()=>this.drain(t.name,e.url,!1),{hint:t.name,keywords:[`undrain`,`rotation`,t.name]}):n(`Actions`,`Drain ${e.url}`,()=>this.drain(t.name,e.url,!0),{hint:t.name,keywords:[`rotation`,`deploy`,t.name]});for(let t of e.caches??[])t.entries&&n(`Actions`,`Purge cache ${t.name}`,()=>this.purgeAll(t.name,t.entries),{...r,keywords:[`clear`,`empty`]});n(`Actions`,`Sign out`,()=>this.logout(),{keywords:[`log out`,`logout`]});for(let e of ra)n(`Go to`,e.label,()=>this.show(e.id),{shortcut:[`G`,e.key.toUpperCase()]});if(i.length>=2&&e.config){let t=i.toLowerCase(),r=Object.entries(e.config).filter(([e,n])=>e.toLowerCase().includes(t)||n.toLowerCase().includes(t)).slice(0,6);for(let[e,t]of r)n(`Settings`,e,()=>{this.filter=e,this.show(`config`)},{hint:t===``?`empty`:t.length>36?`${t.slice(0,35)}…`:t,keywords:[t]})}for(let[e,t]of sa)n(`Docs`,e,()=>window.open(t,`_blank`,`noopener`),{hint:`opens in a new tab`,keywords:[`help`,`documentation`]});return t}show(e,{push:t=!0}={}){this.stop(),this.view=e,this.moreOpen=!1,this.hover=null,location.hash!==`#/${e}`&&(t&&location.hash?history.pushState(null,``,`#/${e}`):history.replaceState(null,``,`#/${e}`)),document.title=`${ra.find(t=>t.id===e)?.label??`Overview`} · rws admin`,this.error=``,this.fieldError={};let n={overview:2e3,upstreams:3e3,certificates:3e4,controls:5e3,audit:1e4}[e];this.load(e),n&&(this.timer=setInterval(()=>this.load(e),n)),e===`logs`&&this.follow(),queueMicrotask(()=>this.querySelector(`main`)?.focus({preventScroll:!0}))}stop(){clearInterval(this.timer),this.timer=void 0,this.abort?.abort(),this.abort=void 0,clearTimeout(this.logTimer),this.logTimer=void 0}async load(e){let t=async(e,t)=>{try{let n=await this.call(t);this.data={...this.data,[e]:n}}catch{}},n=e=>G(e).catch(()=>null);if(e===`overview`){let[,,e]=await Promise.all([t(`status`,`/status`),t(`metrics`,`/metrics`),n(`/logs?n=8`)]);Array.isArray(e)&&(this.recent=e.map(ya).reverse());let r=this.data.metrics;if(typeof r?.requests_total==`number`){let e=Date.now(),t=this.lastTotal?(e-this.lastTotal.at)/1e3:0;if(t>.5){let n=Math.max(0,(r.requests_total-this.lastTotal.total)/t),i=Math.max(0,(r.errors_total??0)-this.lastTotal.errors);this.samples=[...this.samples,{x:e,y:Math.round(n*100)/100,e:i}].slice(-150)}(!this.lastTotal||t>.5)&&(this.lastTotal={total:r.requests_total,errors:r.errors_total??0,at:e})}}else if(e===`upstreams`){let[,e]=await Promise.all([t(`upstreams`,`/upstreams`),n(`/audit?n=200`)]);Array.isArray(e)&&(this.data={...this.data,audit:e})}else e===`certificates`?await t(`certificates`,`/certificates`):e===`controls`?await Promise.all([t(`maintenance`,`/maintenance`),t(`blocklist`,`/blocklist`),t(`flags`,`/flags`),t(`rateLimit`,`/rate-limit`),t(`caches`,`/caches`),this.loadAuthz()]):e===`config`?await t(`config`,`/config`):e===`audit`&&await t(`audit`,`/audit?n=200`)}async follow(){let e=this.abort=new AbortController;this.logAll=[],this.logFrozen=[],this.paused=!1;let t=t=>{if(!e.signal.aborted){for(let e of Array.isArray(t)?t:[t])this.logAll.push(ya(e));this.logAll.length>oa&&this.logAll.splice(0,this.logAll.length-oa),!this.paused&&!this.logTimer&&(this.logTimer=setTimeout(()=>{this.logTimer=void 0,this.logTick++},250))}};try{t(await this.call(`/logs?n=200`)),this.logTick++,await zr(t,e.signal)}catch(t){t.name!==`AbortError`&&!e.signal.aborted&&!this.error&&(this.error=`The live log stopped: ${t.message}`)}}logShown(e){let t=this.logFilter;return t.status!==`all`&&ba(e)!==t.status[0]?!1:!t.text||e.raw.toLowerCase().includes(t.text.toLowerCase())}setLogFilter(e){this.logFilter={...this.logFilter,...e}}setPaused(e){e&&(this.logFrozen=[...this.logAll]),this.paused=e}downloadLog(){let e=this.logAll.filter(e=>this.logShown(e)).map(e=>e.raw),t=URL.createObjectURL(new Blob([`${e.join(`
`)}\n`],{type:`text/plain`})),n=document.createElement(`a`);n.href=t,n.download=`rws-access-${new Date().toISOString().slice(0,19).replace(/[:T]/g,`-`)}.log`,document.body.append(n),n.click(),n.remove(),setTimeout(()=>URL.revokeObjectURL(t),1e3),this.toast(`${$(e.length,`line`,`lines`)} saved`)}async act(e,t,n,r,i=null){this.busy=e;try{let e=await this.call(t,n);return r&&this.toast(r(e),i),await this.load(this.view),!0}catch{return this.error&&=(this.toast(`Didn't work: ${this.error}`),``),!1}finally{this.busy=``}}async drain(e,t,n){let r=((this.data.upstreams??[]).find(t=>t.name===e)?.backends??[]).filter(e=>!e.drained&&e.healthy&&e.url!==t);if(n&&r.length===0&&!await this.confirm({title:`Take ${t} out of rotation?`,body:`It is the last healthy backend of ${e}: every request to ${e} will get 503 until one is back.`,action:`Drain anyway`}))return!1;let i=await this.act(`drain:${t}`,`/upstreams/${encodeURIComponent(e)}/drain`,{method:`PUT`,body:{backend:t,drained:n}},()=>n?`${t} drained: it gets no new requests`:`${t} is back in rotation`,()=>this.drain(e,t,!n));return i&&(this.data={...this.data,upstreams:(this.data.upstreams??[]).map(r=>r.name===e?{...r,backends:r.backends.map(e=>e.url===t?{...e,drained:n}:e)}:r)}),i}async setMaintenance(e){(!e||await this.confirm({title:`Turn on maintenance mode?`,body:`Visitors stop reaching your sites until you turn it off.`,effects:[[`warn`,`Every visitor gets 503 Service Unavailable; browsers see a maintenance page.`],[`ok`,`Health checks keep passing.`],[`ok`,`This console and the admin API keep working.`],[`info`,`It stays on until someone turns it off or the server restarts.`]],type:`maintenance`,action:`Turn on maintenance`,tone:`warn`}))&&await this.act(`maintenance`,`/maintenance`,{method:`PUT`,body:{active:e}},e=>e.active?`Maintenance on: visitors get 503`:`Maintenance off`)&&(this.data={...this.data,status:{...this.data.status,maintenance:e},maintenance:{...this.data.maintenance,active:e}})}reload(){return this.act(`reload`,`/reload`,{method:`POST`},e=>e.applied?`Configuration reloaded`:`Reload kept the running configuration: ${e.errors.join(`; `)}`)}renew(){return this.act(`renew`,`/certificates/renew`,{method:`POST`},()=>`Renewal requested: the new certificate is swapped in when it's issued`)}async blockIp(e){let t=await this.act(`block`,`/blocklist`,{method:`POST`,body:{ip:e}},e=>`${e.blocked} blocked`,()=>this.act(`unblock:${e}`,`/blocklist/${encodeURIComponent(e)}`,{method:`DELETE`},()=>`${e} unblocked`));return t&&this.data.blocklist&&!this.data.blocklist.includes(e)&&(this.data={...this.data,blocklist:[...this.data.blocklist,e]}),t}block(e){e.preventDefault();let t=this.querySelector(`#block-ip`),n=(t?.value??``).trim();n?ua(n)?(this.fieldError={...this.fieldError,block:``},this.blockIp(n).then(e=>{e&&t&&(t.value=``)})):this.fieldError={...this.fieldError,block:`${n} isn't an IPv4 or IPv6 address.`}:this.fieldError={...this.fieldError,block:`Enter an IP address.`}}async unblock(e){let t=await this.act(`unblock:${e}`,`/blocklist/${encodeURIComponent(e)}`,{method:`DELETE`},()=>`${e} unblocked`,()=>this.act(`block`,`/blocklist`,{method:`POST`,body:{ip:e}},()=>`${e} blocked again`));return t&&this.data.blocklist&&(this.data={...this.data,blocklist:this.data.blocklist.filter(t=>t!==e)}),t}async loadAuthz(){try{let[e,t]=await Promise.all([G(`/authz`),G(`/authz/tuples?limit=200`)]);this.data={...this.data,authz:{schema:e,tuples:t.tuples}}}catch(e){e instanceof W&&e.status===404&&(this.data={...this.data,authz:null})}}addTuple(e){e.preventDefault();let t=this.querySelector(`#authz-tuple`),n=(t?.value??``).trim();n&&this.act(`authz-add`,`/authz/tuples`,{method:`POST`,body:{tuple:n}},e=>e.written?`${e.tuple} added`:`${e.tuple} was already there`).then(e=>{e&&t&&(t.value=``)})}async checkAccess(e){e.preventDefault();let t=e=>(this.querySelector(e)?.value??``).trim(),n={subject:t(`#authz-subject`),relation:t(`#authz-relation`),object:t(`#authz-object`)};if(n.subject&&n.relation&&n.object)try{let e=await this.call(`/authz/check`,{method:`POST`,body:n});this.data={...this.data,authzCheck:{...n,...e}}}catch{}}async purgePath(e,t){e.preventDefault();let n=e.target.querySelector(`lit-material-text-field`),r=(n?.value??``).trim();if(!r){this.fieldError={...this.fieldError,[`purge:${t}`]:`Enter a path, e.g. /products/42 or /products/*`};return}this.fieldError={...this.fieldError,[`purge:${t}`]:``};let i=r.endsWith(`*`)?`?prefix=${encodeURIComponent(r.slice(0,-1))}`:`?path=${encodeURIComponent(r)}`;await this.act(`purge:${t}`,`/caches/${encodeURIComponent(t)}${i}`,{method:`DELETE`},e=>`${$(e.purged,`entry`,`entries`)} purged from ${e.name}`)&&n&&(n.value=``)}async purgeAll(e,t){await this.confirm({title:`Purge everything from ${e}?`,body:`All ${t.toLocaleString()} cached ${t===1?`response is`:`responses are`} dropped; the next requests go to the backends until it fills again.`,action:`Purge everything`})&&await this.act(`purge:${e}`,`/caches/${encodeURIComponent(e)}`,{method:`DELETE`},e=>`${$(e.purged,`entry`,`entries`)} purged from ${e.name}`)}async purgeEvery(){let e=(this.data.caches??[]).filter(e=>e.entries),t=e.reduce((e,t)=>e+t.entries,0);if(!await this.confirm({title:`Purge every cache?`,body:`${$(t,`cached response`,`cached responses`)} in ${$(e.length,`cache`,`caches`)} are dropped; the next requests go to the backends until the caches fill again.`,action:`Purge every cache`}))return;this.busy=`purge-every`;let n=0;try{for(let t of e)n+=(await this.call(`/caches/${encodeURIComponent(t.name)}`,{method:`DELETE`})).purged;this.toast(`${$(n,`entry`,`entries`)} purged from ${$(e.length,`cache`,`caches`)}`)}catch{this.toast(`Didn't work: ${this.error||`the server didn't answer`}`),this.error=``}finally{this.busy=``,this.load(this.view)}}backendHealth(){let e=(this.data.upstreams??[]).flatMap(e=>e.backends);return[e.filter(e=>e.healthy&&!e.drained).length,e.length]}serverState(){let e=this.data.status;if(this.conn.lost)return{tone:`bad`,label:`Disconnected`,detail:this.conn.at?`last answer ${ha(this.conn.at)}`:`retrying`,view:`overview`,title:`The admin API doesn't answer; retrying every 5 seconds`};if(!e)return null;if(e.maintenance)return{tone:`warn`,label:`Maintenance`,detail:`visitors get 503`,view:`controls`};let[t,n]=this.backendHealth();return n>0&&t<n?{tone:t===0?`bad`:`warn`,label:`Degraded`,detail:`${t} of ${n} backends serving`,view:`upstreams`}:e.ready?{tone:`ok`,label:`Ready`,detail:n?`${n} of ${n} backends serving`:`up ${pa(e.uptime_secs)}`,view:`overview`}:{tone:`bad`,label:`Not ready`,detail:`readiness checks fail`,view:`overview`}}attention(){let e=this.data,t=[];e.status&&!e.status.ready&&!e.status.maintenance&&t.push({tone:`bad`,title:`The server reports not ready`,detail:`Load balancers that probe readiness stop sending it traffic.`});for(let n of e.upstreams??[])for(let e of n.backends){let r={label:`Open Upstreams`,run:()=>this.show(`upstreams`)};e.healthy?e.breaker===`open`?t.push({tone:`bad`,title:`${e.url}: circuit breaker open`,detail:`Upstream ${n.name}: too many failures, requests are held back until a probe succeeds.`,action:r}):e.breaker===`half_open`&&t.push({tone:`warn`,title:`${e.url}: circuit breaker probing`,detail:`Upstream ${n.name}: a few requests are let through to see if it recovered.`,action:r}):t.push({tone:`bad`,title:`${e.url} is failing health checks`,detail:`Upstream ${n.name}: it gets no requests until the checks pass.`,action:r}),e.drained&&t.push({tone:`warn`,title:`${e.url} is drained`,detail:`Upstream ${n.name}: out of rotation until you put it back.`,action:{label:`Put back`,busy:`drain:${e.url}`,run:()=>this.drain(n.name,e.url,!1)}})}let n=e.certificates?.acme?.running;for(let r of e.certificates?.certificates??[])r.days_left>=21||t.push({tone:r.days_left<7?`bad`:`warn`,title:r.days_left<0?`The certificate for ${r.name} has expired`:`The certificate for ${r.name} expires in ${$(r.days_left,`day`,`days`)}`,detail:n?`ACME renews it by itself; you can also renew now.`:`Replace the certificate files, then reload the configuration.`,action:n?{label:`Renew now`,busy:`renew`,run:()=>this.renew()}:{label:`Open Certificates`,run:()=>this.show(`certificates`)}});for(let n of e.status?.disk??[])n.low&&t.push({tone:`warn`,title:`Disk almost full: ${n.dir}`,detail:`${Ur(n.free_bytes)} free of ${Ur(n.total_bytes)}. rws has stopped writing the access log and disk cache there.`});let r={bad:0,warn:1};return t.sort((e,t)=>r[e.tone]-r[t.tone])}renderSide(){let e=this.data.status;return h`<aside class="side">
      <a class="brand" href="#/overview" aria-label="rws admin, Overview">${ma}<span>rws</span>${e?h`<span class="ver">${e.version}</span>`:v}</a>
      <nav aria-label="Sections">
        ${ia.map(e=>h`<div class="nav-group" role="group" aria-label=${e}>
          <div class="nav-label" aria-hidden="true">${e}</div>
          ${ra.filter(t=>t.group===e).map(e=>h`<a class="nav-link" href="#/${e.id}" aria-current=${this.view===e.id?`page`:v} aria-keyshortcuts="G ${e.key.toUpperCase()}">
            <span class="icon">${Z(e.icon)}</span>${e.label}<kbd aria-hidden="true">G ${e.key.toUpperCase()}</kbd></a>`)}
        </div>`)}
      </nav>
      ${this.me?h`<div class="side-foot">
        <span class="who" title="Signed in ${this.me.via===`oidc`?`with single sign-on`:`with a token`}">${this.me.user} · ${this.me.role}</span>
        <button class="btn btn-quiet" @click=${()=>this.logout()}><span class="icon">${Z(`sign-out`)}</span>Sign out</button>
      </div>`:v}
    </aside>`}renderTop(){let e=this.serverState();return h`<header class="top">
      <span class="top-brand">${ma}<span class="top-title">${ra.find(e=>e.id===this.view)?.label??`Overview`}</span></span>
      ${e?h`<button class="state" data-tone=${e.tone} title=${e.title??`The server's state. Click to see why.`} @click=${()=>this.show(e.view)}>
        <span class="dot" aria-hidden="true"></span><span>${e.label}</span>${e.detail?h`<span class="detail">${e.detail}</span>`:v}</button>`:v}
      <button class="cmd" aria-label="Search or run a command" aria-keyshortcuts="Control+K Meta+K" @click=${()=>this.openPalette()}>
        <span class="icon">${Z(`magnifying-glass`)}</span><span class="cmd-text">Search or run a command</span><kbd aria-hidden="true">${la} K</kbd></button>
      <rws-ago class="updated" .at=${this.conn.at}></rws-ago>
    </header>`}renderTabbar(){let e=!aa.includes(this.view);return h`<nav class="tabbar" aria-label="Sections">
        ${aa.map(e=>ra.find(t=>t.id===e)).map(e=>h`<a href="#/${e.id}" aria-current=${this.view===e.id?`page`:v}><span class="icon">${Z(e.icon)}</span>${e.label}</a>`)}
        <button class=${e?`is-current`:``} aria-expanded=${this.moreOpen} aria-controls="more-sheet" @click=${()=>this.toggleMore()}><span class="icon">${Z(`dots-three`)}</span>More</button>
      </nav>
      ${this.moreOpen?h`<div class="more-scrim" @click=${()=>this.moreOpen=!1}></div>
          <nav id="more-sheet" class="more-sheet" aria-label="More sections">
            ${ra.filter(e=>!aa.includes(e.id)).map(e=>h`<a href="#/${e.id}" aria-current=${this.view===e.id?`page`:v}
              @click=${()=>this.moreOpen=!1}><span class="icon">${Z(e.icon)}</span>${e.label}</a>`)}
            ${this.me?h`<div class="more-foot"><span class="who">${this.me.user} · ${this.me.role}</span>
              <button class="btn btn-quiet" @click=${()=>this.logout()}><span class="icon">${Z(`sign-out`)}</span>Sign out</button></div>`:v}
          </nav>`:v}`}toggleMore(){this.moreOpen=!this.moreOpen,this.moreOpen&&this.updateComplete.then(()=>this.querySelector(`.more-sheet a`)?.focus())}render(){if(!this.authed)return this.renderLogin();let e=this.data.status,t=this.ask;return h`
      <div class="shell">
        ${this.renderSide()}
        <div class="body">
          ${this.renderTop()}
          <main id="main" tabindex="-1">
            ${e?.maintenance?h`<div class="banner" role="status">
                  <span class="banner-title">Maintenance mode is on</span>
                  <span class="banner-text">Visitors get 503 Service Unavailable until you turn it off.</span>
                  <button class="btn btn-warn-fill" ?disabled=${this.busy===`maintenance`} @click=${()=>this.setMaintenance(!1)}>Turn off</button>
                </div>`:v}
            ${this.error?h`<lit-material-alert variant="error"><span slot="title">Couldn't load this page</span>${this.error}
                <lit-material-button variant="text" @click=${()=>this.load(this.view)}>Retry</lit-material-button></lit-material-alert>`:v}
            ${this.renderView()}
          </main>
        </div>
      </div>
      ${this.renderTabbar()}
      <div class="toast" role="status" aria-live="polite">
        ${this.notice?h`<div class="toast-body"><span>${this.notice.message}</span>
              ${this.notice.undo?h`<button class="toast-action" @click=${()=>{let e=this.notice.undo;this.notice=null,e()}}>Undo</button>`:v}
              <button class="toast-close" aria-label="Dismiss" @click=${()=>this.notice=null}>✕</button></div>`:v}
      </div>
      <lit-material-command-palette label="Search or run a command" placeholder="Search or run a command" .items=${this.paletteItems()}
        .hints=${ca}
        @input=${e=>this.query=e.composedPath()[0]?.value??``}
        @close=${()=>this.query=``}
        @select=${e=>{this.query=``,e.detail.item.run?.()}}></lit-material-command-palette>
      <dialog id="confirm" class="confirm" role="alertdialog" data-tone=${t?.tone??`danger`} aria-labelledby="confirm-title" aria-describedby="confirm-body"
        @cancel=${e=>{e.preventDefault(),this.answer(!1)}}>
        ${t?h`<form novalidate @submit=${e=>{e.preventDefault(),this.confirmReady()&&this.answer(!0)}}>
            <h2 id="confirm-title">${t.title}</h2>
            <p id="confirm-body">${t.body}</p>
            ${t.effects?h`<ul class="effects">${t.effects.map(([e,t])=>h`<li><span class="sq" data-tone=${e}></span><span>${t}</span></li>`)}</ul>`:v}
            ${t.type?h`<lit-material-text-field id="confirm-type" variant="outlined" label="Type ${t.type} to confirm" autocomplete="off" autocapitalize="none" spellcheck="false"
                  @input=${e=>this.typed=e.target.value??``}></lit-material-text-field>`:v}
            <div class="confirm-actions">
              <lit-material-button class="cancel" type="button" variant="text" @click=${()=>this.answer(!1)}>Cancel</lit-material-button>
              <lit-material-button class="go" type="submit" ?disabled=${!this.confirmReady()}>${t.action}</lit-material-button>
            </div>
          </form>`:v}
      </dialog>
    `}renderLogin(){return h`
      <div class="login">
        <section class="login-card card" aria-labelledby="login-title">
          <div class="login-brand">${ma}<span>rws admin</span></div>
          <h1 id="login-title">Sign in</h1>
          ${this.sso?h`<lit-material-button href="/admin/auth/login" style="width: 100%">Sign in with single sign-on</lit-material-button>
            <p class="login-or">or with a token</p>`:v}
          <p>Paste an admin token: the value of <code>RWS_ADMIN_TOKEN</code> (or the variable your <code>[admin] token_env</code> names), or your own from <code>[admin] users</code>. It stays in this tab only.</p>
          <form @submit=${e=>this.login(e)} novalidate>
            <lit-material-text-field id="token" class="token" variant="outlined" label="Admin token" type="password" autocomplete="current-password" required autofocus
              ?error=${!!this.loginError} error-text=${this.loginError??``}></lit-material-text-field>
            <lit-material-button type="submit">Sign in</lit-material-button>
          </form>
        </section>
      </div>
    `}renderView(){switch(this.view){case`upstreams`:return this.renderUpstreams();case`certificates`:return this.renderCertificates();case`controls`:return this.renderControls();case`traffic`:return h`<rws-traffic></rws-traffic>`;case`config`:return this.renderConfig();case`logs`:return this.renderLogs();case`audit`:return this.renderAudit();default:return this.renderOverview()}}head(e,t=v){return h`<div class="page-head"><h1>${e}</h1>${t===v?v:h`<span class="sub">${t}</span>`}</div>`}skeleton(e=3){return h`<div class="skeleton" aria-busy="true">${Array.from({length:e},()=>h`<div class="card"><lit-material-skeleton width="40%"></lit-material-skeleton><lit-material-skeleton variant="rounded" height="32px"></lit-material-skeleton></div>`)}</div>`}renderOverview(){let e=this.data.status,t=this.data.metrics;if(!e||!t)return h`${this.head(`Overview`)}${this.skeleton(4)}`;let n=t.requests_total?t.errors_total/t.requests_total*100:0,r=this.samples,i=r.at(-1),a=r.length?Math.max(...r.map(e=>e.y)):0,o=r.length?r.reduce((e,t)=>e+t.y,0)/r.length:0,s=r.length>1?(i.x-r[0].x)/1e3:0,c=s>=90?`${Math.round(s/60)} min`:`${Math.round(s)} s`,l=this.attention();return h`
      ${this.head(`Overview`,h`Up ${pa(e.uptime_secs)}. Numbers refresh every 2 seconds; this console's own requests aren't counted.`)}
      <section class="card pulse" aria-labelledby="pulse-title">
        <div class="pulse-top">
          <div class="pulse-main">
            <span class="pulse-label" id="pulse-title">Requests per second</span>
            <span class="pulse-big">${i?ga(i.y):`–`}</span>
            <span class="pulse-cmp">${r.length>1?h`Peak ${ga(a)}, average ${ga(o)} over the last ${c}.`:`Measuring: the first number arrives in a few seconds.`}</span>
          </div>
          <dl class="vitals">
            <div class="vital"><dt>Errors</dt><dd>${n.toFixed(2)}%</dd>
              <span class="note" data-tone=${n>=5?`bad`:n>=1?`warn`:`none`}>${t.errors_total.toLocaleString()} since start</span></div>
            <div class="vital"><dt>Open connections</dt><dd>${t.active_connections.toLocaleString()}</dd><span class="note">right now</span></div>
            <div class="vital"><dt>Queued for a worker</dt><dd>${t.thread_pool_queued.toLocaleString()}</dd>
              <span class="note" data-tone=${t.thread_pool_queued>0?`warn`:`none`}>${t.thread_pool_queued>0?`waiting for a free thread`:`nothing waits`}</span></div>
            <div class="vital"><dt>Requests served</dt><dd>${t.requests_total.toLocaleString()}</dd><span class="note">since start</span></div>
          </dl>
        </div>
        ${this.renderChart()}
      </section>
      <div class="cols">
        <div class="stack">
          <section class="card" aria-labelledby="attn-title">
            <div class="card-head"><h2 id="attn-title">Needs attention</h2>${l.length?h`<span class="count">${l.length}</span>`:v}</div>
            ${l.length===0?h`<div class="all-clear"><span class="sq" data-tone="ok"></span>Nothing needs you: backends, certificates and disk are fine.</div>`:l.map(e=>h`<div class="attn">
                  <span class="sq" data-tone=${e.tone}></span>
                  <div class="attn-text"><span class="attn-title">${e.title}</span><span class="attn-detail">${e.detail}</span></div>
                  ${e.action?h`<button class="btn" ?disabled=${!!e.action.busy&&this.busy===e.action.busy} @click=${e.action.run}>${e.action.label}</button>`:v}
                </div>`)}
          </section>
          <section class="card" aria-labelledby="live-title">
            <div class="card-head"><h2 id="live-title">Live requests</h2>
              <span class="meta">The newest eight. This console's own aren't logged.</span><span class="spacer"></span>
              <a class="link" href="#/logs">Open Logs</a></div>
            <div class="reqs" role="table" aria-labelledby="live-title">
              ${wa}
              ${this.recent?this.recent.length===0?h`<div class="req-none">No requests yet. They appear here as they arrive.</div>`:this.recent.map(Ca):h`<div class="req-none">Loading…</div>`}
            </div>
          </section>
        </div>
        <section class="card" aria-labelledby="server-title">
          <div class="card-head"><h2 id="server-title">Server</h2></div>
          <dl class="facts">
            <dt>Main listener</dt><dd>${e.listeners.main.address} <span class="plain">${e.listeners.main.tls?`HTTPS, HTTP/2, HTTP/3`:`plain HTTP`}</span></dd>
            <dt>HTTP redirect</dt><dd>${e.listeners.redirect??h`<span class="plain">off</span>`}</dd>
            <dt>Admin</dt><dd>${e.listeners.admin}</dd>
            <dt>Uptime</dt><dd>${pa(e.uptime_secs)}</dd>
            <dt>Process</dt><dd>${e.pid}</dd>
            <dt>Version</dt><dd>${e.version}</dd>
            <dt>Built with</dt><dd><span class="plain">${e.features.join(`, `)}</span></dd>
          </dl>
        </section>
      </div>
    `}renderChart(){let e=this.samples;if(e.length<2)return h`<div class="chart-empty">The chart fills in as this page stays open.</div>`;let t=1e3,n=e[0].x,r=Math.max(1,e.at(-1).x-n),i=va(Math.max(...e.map(e=>e.y))),a=e=>(e.x-n)/r*t,o=e=>16+(1-e/i)*176,s=e.map((e,t)=>`${t?`L`:`M`}${a(e).toFixed(1)} ${o(e.y).toFixed(1)}`).join(``),c=e.filter(e=>e.e>0),l=this.hover!==null&&e[this.hover]?e[this.hover]:null,u=Math.max(...e.map(e=>e.y)),ee=t=>{let i=t.currentTarget.getBoundingClientRect(),a=n+Math.min(1,Math.max(0,(t.clientX-i.left)/i.width))*r,o=0;for(let t=1;t<e.length;t++)Math.abs(e[t].x-a)<Math.abs(e[o].x-a)&&(o=t);o!==this.hover&&(this.hover=o)},te=(e,t)=>`${(e/t*100).toFixed(2)}%`;return h`
      <div class="chart" @pointermove=${ee} @pointerdown=${ee} @pointerleave=${()=>this.hover=null}>
        <svg viewBox="0 0 ${t} ${200}" preserveAspectRatio="none" role="img"
          aria-label="Requests per second over the last ${Math.round(r/1e3)} seconds: now ${ga(e.at(-1).y)}, peak ${ga(u)}${c.length?`, errors in ${$(c.length,`sample`,`samples`)}`:``}">
          ${[0,.5,1].map(e=>g`<line class="grid-line" x1="0" x2=${t} y1=${o(i*e)} y2=${o(i*e)} vector-effect="non-scaling-stroke"></line>`)}
          <path class="area" d="${s}L${t} ${o(0)}L0 ${o(0)}Z"></path>
          <path class="line" d=${s} vector-effect="non-scaling-stroke"></path>
          ${c.map(e=>g`<rect class="err" x=${a(e)-2} y=${194} width="4" height="6"></rect>`)}
          ${l?g`<line class="cursor" x1=${a(l)} x2=${a(l)} y1="0" y2=${200} vector-effect="non-scaling-stroke"></line>`:v}
        </svg>
        <span class="chart-y" style="top: ${te(o(i),200)}">${ga(i)}</span>
        <span class="chart-y" style="top: ${te(o(i/2),200)}">${ga(i/2)}</span>
        ${l?h`<span class="chart-dot" style="left: ${te(a(l),t)}; top: ${te(o(l.y),200)}"></span>
          <span class="chart-tip" style="left: clamp(90px, ${te(a(l),t)}, calc(100% - 90px))">${ha(l.x)} · ${ga(l.y)} req/s${l.e?` · ${$(l.e,`error`,`errors`)}`:``}</span>`:v}
      </div>
      <div class="chart-x" aria-hidden="true"><span>${ha(n)}</span>
        ${c.length?h`<span class="err-key"><span class="sq" data-tone="bad"></span>errors</span>`:v}<span>now</span></div>
    `}renderUpstreams(){let e=this.data.upstreams;if(!e)return h`${this.head(`Upstreams`)}${this.skeleton(2)}`;let t=e.reduce((e,t)=>e+t.backends.length,0),n=(this.data.audit??[]).filter(e=>e.path.includes(`/upstreams/`)&&e.status<300).slice(0,6);return h`
      ${this.head(`Upstreams`,e.length?h`${$(e.length,`upstream`,`upstreams`)}, ${$(t,`backend`,`backends`)}. Drain a backend before deploying it: new requests go elsewhere, requests already on it finish.`:v)}
      ${e.length===0?h`<div class="card"><lit-material-empty-state size="compact" headline="No upstreams configured">Upstreams come from <code>[[upstream]]</code> sections in <code>rws.config.toml</code>. Add one, then reload from Controls.</lit-material-empty-state></div>`:h`<div class="cols">
          <div class="stack">${e.map(e=>this.renderUpstream(e))}</div>
          <section class="card" aria-labelledby="changes-title">
            <div class="card-head"><h2 id="changes-title">Recent changes</h2><span class="spacer"></span><a class="link" href="#/audit">Open Audit</a></div>
            ${n.length===0?h`<div class="all-clear">Nobody has drained or put back a backend since the server started.</div>`:n.map(e=>h`<div class="change"><span class="change-what">${Ta(e)}</span>
                  <span class="change-who">${e.user} · ${_a(e.at*1e3)}</span></div>`)}
          </section>
        </div>`}
    `}renderUpstream(e){let t=e.backends.filter(e=>e.healthy&&!e.drained),n=t.reduce((e,t)=>e+t.weight,0);return h`<section class="card" aria-labelledby="up-${e.name}">
      <div class="card-head"><h2 id="up-${e.name}" class="mono">${e.name}</h2>
        <span class="meta">${t.length} of ${e.backends.length} serving</span></div>
      <div class="table-wrap"><table class="tbl tbl-stack" role="table">
        <thead role="rowgroup"><tr role="row"><th role="columnheader">Backend</th><th role="columnheader">Share of traffic</th><th role="columnheader">Health</th><th role="columnheader">Breaker</th><th role="columnheader"><span class="sr">Rotation</span></th></tr></thead>
        <tbody role="rowgroup">${e.backends.map(t=>{let r=t.healthy&&!t.drained&&n?Math.round(t.weight/n*100):0;return h`<tr role="row" class=${t.drained?`is-warn`:``}>
            <td role="cell" class="mono nowrap">${t.url}</td>
            <td role="cell" class="wide"><div class="share"><span class="share-track"><span class="share-fill" style="width: ${r}%"></span></span>
              <span class="share-text">${r}%</span>${t.drained?Q(`warning`,`Drained`):h`<span class="share-note">weight ${t.weight}</span>`}</div></td>
            <td role="cell">${t.healthy?Q(`ok`,`Healthy`):Q(`error`,`Failing checks`)}</td>
            <td role="cell" data-label="Breaker">${t.breaker===null?Q(`neutral`,`None`):t.breaker===`closed`?Q(`ok`,`Closed`):t.breaker===`half_open`?Q(`warning`,`Probing`):Q(`error`,`Open`)}</td>
            <td role="cell" class="act"><button class="btn" aria-label="${t.drained?`Put back`:`Drain`} ${t.url}" ?disabled=${this.busy===`drain:${t.url}`}
                @click=${()=>this.drain(e.name,t.url,!t.drained)}>${t.drained?`Put back`:`Drain`}</button></td>
          </tr>`})}</tbody>
      </table></div>
    </section>`}renderCertificates(){let e=this.data.certificates;if(!e)return h`${this.head(`Certificates`)}${this.skeleton(2)}`;let t=e.acme,n=e=>e.days_left<7?Q(`error`,String(e.days_left)):e.days_left<21?Q(`warning`,String(e.days_left)):e.days_left;return h`
      ${this.head(`Certificates`,`Every certificate this server presents, soonest expiry first.`)}
      <section class="card" aria-labelledby="certs-title">
        <div class="card-head"><h2 id="certs-title">Presented</h2>${e.certificates.length?h`<span class="count">${e.certificates.length}</span>`:v}</div>
        ${e.certificates.length===0?h`<lit-material-empty-state size="compact" headline="No TLS certificates">This server speaks plain HTTP. Set <code>tls_cert_file</code> and <code>tls_key_file</code>, or <code>[acme] domains</code>, to serve HTTPS.</lit-material-empty-state>`:h`<div class="table-wrap"><table class="tbl">
              <thead><tr><th>Name</th><th>Expires</th><th class="num">Days left</th><th>Source</th></tr></thead>
              <tbody>${e.certificates.map(e=>h`<tr><td class="mono">${e.name}</td><td>${e.expires}</td><td class="num">${n(e)}</td>
                <td class="dim">${e.source===`on_demand`?`On demand`:`Configured`}</td></tr>`)}</tbody>
            </table></div>`}
      </section>
      <section class="card" aria-labelledby="acme-title">
        <div class="card-head"><h2 id="acme-title">ACME</h2>${t.running?Q(`ok`,`Renews by itself`):Q(`neutral`,`Off`)}</div>
        <div class="card-body">
          ${t.running?h`<p>${t.last_check?h`Last check ${new Date(t.last_check.at*1e3).toLocaleString()}: ${t.last_check.outcome}.`:`No check has finished yet.`}</p>
                <div class="row"><button class="btn" ?disabled=${this.busy===`renew`} @click=${()=>this.renew()}><span class="icon">${Z(`arrow-clockwise`)}</span>Renew now</button></div>`:h`<p>Set <code>[acme] domains</code> to have rws obtain and renew certificates itself.</p>`}
        </div>
      </section>
    `}renderControls(){let e=this.data;if(!e.maintenance||!e.blocklist||!e.flags)return h`${this.head(`Controls`)}${this.skeleton(3)}`;let t=e.maintenance.active,n=e.caches??[];return h`
      ${this.head(`Controls`,`Things that change what visitors get, right now. They last until the server restarts.`)}
      <div class="trio">
        <section class="card" aria-labelledby="maint-title">
          <div class="card-head"><h2 id="maint-title">Maintenance mode</h2></div>
          <div class="card-body">
            <p>${t?Q(`warning`,`On: 503 for visitors`):`Off. Visitors reach every host as usual.`}</p>
            <div class="row"><button id="maintenance-toggle" class="btn ${t?``:`btn-warn`}" ?disabled=${this.busy===`maintenance`} @click=${()=>this.setMaintenance(!t)}>${t?`Turn off`:`Turn on`}</button></div>
          </div>
        </section>
        <section class="card" aria-labelledby="reload-title">
          <div class="card-head"><h2 id="reload-title">Configuration</h2></div>
          <div class="card-body">
            <p>Re-read <code>rws.config.toml</code> and the certificates. A file with errors is refused and the running configuration stays.</p>
            <div class="row"><button class="btn" ?disabled=${this.busy===`reload`} @click=${()=>this.reload()}><span class="icon">${Z(`arrow-clockwise`)}</span>Reload</button></div>
          </div>
        </section>
        <section class="card" aria-labelledby="limit-title">
          <div class="card-head"><h2 id="limit-title">Global rate limit</h2></div>
          <div class="card-body">
            ${e.rateLimit?.configured?h`<span class="big-value">${e.rateLimit.max_requests.toLocaleString()} per ${e.rateLimit.window_secs} s</span><p>Requests from one address above that get 429.</p>`:h`<span class="big-value">Off</span><p>Set <code>rate_limit</code> in the config to cap requests per address.</p>`}
          </div>
        </section>
      </div>

      <section class="card" aria-labelledby="caches-title">
        <div class="card-head"><h2 id="caches-title">Response caches</h2><span class="spacer"></span>
          ${n.length>1?h`<button class="btn btn-danger" ?disabled=${this.busy===`purge-every`||!n.some(e=>e.entries)} @click=${()=>this.purgeEvery()}>Purge every cache</button>`:v}</div>
        ${n.length===0?h`<lit-material-empty-state size="compact" headline="No response cache">Add <code>[route.middleware.cache]</code> to a route; its cache appears here to purge.</lit-material-empty-state>`:h`<div class="table-wrap"><table class="tbl tbl-stack" role="table">
              <thead role="rowgroup"><tr role="row"><th role="columnheader">Cache</th><th role="columnheader">Kept in</th><th role="columnheader" class="num">Entries</th><th role="columnheader" class="num">On disk</th><th role="columnheader">Served from cache</th><th role="columnheader"><span class="sr">Purge</span></th></tr></thead>
              <tbody role="rowgroup">${n.map(e=>{let t=e.hits+e.misses,n=t?Math.round(e.hits/t*100):0;return h`<tr role="row">
                  <td role="cell" class="mono">${e.name}</td>
                  <td role="cell" class="dim" data-label="Kept in">${e.kind}</td>
                  <td role="cell" class="num" data-label="Entries">${e.entries.toLocaleString()}</td>
                  <td role="cell" class="num" data-label="On disk">${e.bytes===null?`–`:Ur(e.bytes)}</td>
                  <td role="cell" class="wide">${t?h`<div class="share" title="${e.hits.toLocaleString()} hits, ${e.misses.toLocaleString()} misses"><span class="share-track"><span class="share-fill" data-tone="ok" style="width: ${n}%"></span></span><span class="share-text">${n}%</span></div>`:h`<span class="dim">no requests yet</span>`}</td>
                  <td role="cell" class="act"><button class="btn btn-danger" aria-label="Purge all of ${e.name}" ?disabled=${this.busy===`purge:${e.name}`||!e.entries} @click=${()=>this.purgeAll(e.name,e.entries)}>Purge all</button></td>
                </tr>`})}</tbody>
            </table></div>
            <div class="card-body">
              ${n.map(e=>h`<form class="field-row purge" @submit=${t=>this.purgePath(t,e.name)} novalidate>
                <lit-material-text-field variant="outlined" label="Path to purge from ${e.name}" supporting-text="One path, or a prefix ending in *: /products/*"
                  ?error=${!!this.fieldError[`purge:${e.name}`]} error-text=${this.fieldError[`purge:${e.name}`]??``}></lit-material-text-field>
                <lit-material-button type="submit" variant="tonal" ?disabled=${this.busy===`purge:${e.name}`}>Purge path</lit-material-button>
              </form>`)}
            </div>`}
      </section>

      <div class="cols">
        <section class="card" aria-labelledby="block-title">
          <div class="card-head"><h2 id="block-title">Blocked addresses</h2>${e.blocklist.length?h`<span class="count">${e.blocklist.length}</span>`:v}</div>
          <div class="card-body">
            <form class="field-row" @submit=${e=>this.block(e)} novalidate>
              <lit-material-text-field id="block-ip" variant="outlined" label="IP address" supporting-text="IPv4 or IPv6, e.g. 203.0.113.7"
                ?error=${!!this.fieldError.block} error-text=${this.fieldError.block??``} @input=${()=>this.fieldError.block&&(this.fieldError={...this.fieldError,block:``})}></lit-material-text-field>
              <lit-material-button type="submit" variant="tonal" ?disabled=${this.busy===`block`}><span slot="icon" class="icon">${Z(`plus`)}</span>Block</lit-material-button>
            </form>
            ${e.blocklist.length===0?h`<p>Nobody is blocked. A blocked address gets 403 on every request.</p>`:v}
          </div>
          ${e.blocklist.length===0?v:h`<div class="table-wrap"><table class="tbl">
            <thead><tr><th>Address</th><th><span class="sr">Unblock</span></th></tr></thead>
            <tbody>${e.blocklist.map(e=>h`<tr><td class="mono">${e}</td>
              <td class="act"><button class="btn" aria-label="Unblock ${e}" ?disabled=${this.busy===`unblock:${e}`} @click=${()=>this.unblock(e)}>Unblock</button></td></tr>`)}</tbody>
          </table></div>`}
        </section>
        <section class="card" aria-labelledby="flags-title">
          <div class="card-head"><h2 id="flags-title">Feature flags</h2></div>
          ${e.flags.length===0?h`<lit-material-empty-state size="compact" headline="No feature flags">Applications register flags with <code>feature::global()</code>; they appear here to switch at runtime.</lit-material-empty-state>`:h`<div class="table-wrap"><table class="tbl">
                <thead><tr><th>Flag</th><th><span class="sr">Enabled</span></th></tr></thead>
                <tbody>${e.flags.map(e=>h`<tr><td class="mono">${e.name}</td>
                  <td class="act"><lit-material-switch aria-label="${e.name}" .checked=${e.enabled}
                    @change=${t=>this.act(`flag:${e.name}`,`/flags/${encodeURIComponent(e.name)}`,{method:`PUT`,body:{enabled:t.target.checked}},e=>`${e.name} ${e.enabled?`on`:`off`}`)}></lit-material-switch></td></tr>`)}</tbody>
              </table></div>`}
        </section>
      </div>

      ${e.authz?this.renderAuthz(e.authz,e.authzCheck):v}
    `}renderAuthz(e,t){return h`
      <section class="card" aria-labelledby="authz-title">
        <div class="card-head"><h2 id="authz-title">Authorization</h2><span class="meta">Relationship tuples, object#relation@subject. Schema: ${Object.entries(e.schema.types).filter(([,e])=>e.length).map(([e,t])=>`${e}: ${t.join(`, `)}`).join(` · `)}.</span></div>
        <div class="card-body">
          <form class="field-row" @submit=${e=>this.addTuple(e)} novalidate>
            <lit-material-text-field id="authz-tuple" variant="outlined" label="Tuple" supporting-text="e.g. doc:42#viewer@user:alice"></lit-material-text-field>
            <lit-material-button type="submit" variant="tonal" ?disabled=${this.busy===`authz-add`}><span slot="icon" class="icon">${Z(`plus`)}</span>Add</lit-material-button>
          </form>
          ${e.tuples.length===0?h`<p>No tuples yet.</p>`:v}
        </div>
        ${e.tuples.length===0?v:h`<div class="table-wrap"><table class="tbl">
          <thead><tr><th>Tuple</th><th><span class="sr">Remove</span></th></tr></thead>
          <tbody>${e.tuples.map(e=>h`<tr><td class="mono">${e}</td>
            <td class="act"><button class="btn" aria-label="Remove ${e}" ?disabled=${this.busy===`authz-del:${e}`}
              @click=${()=>this.act(`authz-del:${e}`,`/authz/tuples?tuple=${encodeURIComponent(e)}`,{method:`DELETE`},()=>`${e} removed`)}>Remove</button></td></tr>`)}</tbody>
        </table></div>`}
        <div class="card-body">
          <form class="field-row" @submit=${e=>this.checkAccess(e)} novalidate>
            <lit-material-text-field id="authz-subject" variant="outlined" label="Subject" supporting-text="user:alice"></lit-material-text-field>
            <lit-material-text-field id="authz-relation" variant="outlined" label="Relation" supporting-text="viewer"></lit-material-text-field>
            <lit-material-text-field id="authz-object" variant="outlined" label="Object" supporting-text="doc:42"></lit-material-text-field>
            <lit-material-button type="submit" variant="tonal">Check</lit-material-button>
          </form>
          ${t?h`<p class="result" id="authz-result">
                ${t.allowed?Q(`ok`,`Allowed`):Q(`error`,`Denied`)}
                <span>${t.subject} ${t.relation} ${t.object}${t.allowed?h`, through <code>${t.path.join(` → `)}</code>`:v}</span></p>`:v}
        </div>
      </section>
    `}renderConfig(){let e=this.data.config;if(!e)return h`${this.head(`Config`)}${this.skeleton(2)}`;let t=this.filter.toLowerCase(),n=Object.entries(e),r=n.filter(([e,n])=>!t||e.toLowerCase().includes(t)||n.toLowerCase().includes(t));return h`
      ${this.head(`Config`,h`Every setting in effect. Secrets read (hidden). Change them in <code>rws.config.toml</code> or the environment, then reload from Controls.`)}
      <section class="card" aria-labelledby="config-title">
        <div class="card-head"><h2 id="config-title">Settings in effect</h2><span class="count">${t?`${r.length} of ${n.length}`:n.length}</span></div>
        <div class="card-body">
          <lit-material-text-field class="filter" variant="outlined" label="Filter settings" .value=${this.filter} @input=${e=>this.filter=e.target.value}></lit-material-text-field>
        </div>
        ${r.length===0?h`<lit-material-empty-state size="compact" headline="No setting matches “${this.filter}”">Clear the filter to see all ${n.length}.</lit-material-empty-state>`:h`<div class="table-wrap"><table class="tbl">
              <thead><tr><th>Setting</th><th>Value</th></tr></thead>
              <tbody>${r.map(([e,t])=>h`<tr><td class="mono">${e}</td>
                <td class="mono">${t===``?Q(`neutral`,`empty`):h`<lit-material-truncate position="middle" style="max-width: min(60ch, 50vw)" .value=${t}></lit-material-truncate>`}</td></tr>`)}</tbody>
            </table></div>`}
      </section>
    `}renderAudit(){let e=this.data.audit;if(!e)return h`${this.head(`Audit`)}${this.skeleton(2)}`;let t=e=>e.status<300?Q(`ok`,`Done`):e.status===401||e.status===403?Q(`error`,`Refused`):Q(`warning`,String(e.status));return h`
      ${this.head(`Audit`,h`Every change made through the admin API, this console or MCP tools, and every refused attempt, newest first. Kept in memory (the last 1000); <code>[admin] audit_log</code> also writes them to a file.`)}
      <section class="card" aria-labelledby="audit-title">
        <div class="card-head"><h2 id="audit-title">Changes</h2>${e.length?h`<span class="count">${e.length}</span>`:v}</div>
        ${e.length===0?h`<lit-material-empty-state size="compact" headline="No changes yet">Changes appear here as people reload, drain, block or purge.</lit-material-empty-state>`:h`<div class="table-wrap"><table class="tbl">
              <thead><tr><th>When</th><th>Who</th><th>Change</th><th>Details</th><th>Result</th><th>From</th></tr></thead>
              <tbody>${e.map(e=>h`<tr>
                <td class="dim" style="white-space: nowrap">${new Date(e.at*1e3).toLocaleString()}</td>
                <td>${e.user} <span class="meta">${e.role}${e.via===`oidc`?` · SSO`:e.via===`mcp`?` · MCP`:``}</span></td>
                <td class="mono">${e.method===`TOOL`?`tool`:e.method} ${e.path.replace(`/admin/api/v1`,``)}</td>
                <td class="mono">${e.detail?h`<lit-material-truncate style="max-width: min(40ch, 30vw)" .value=${e.detail}></lit-material-truncate>`:v}</td>
                <td>${t(e)}</td>
                <td class="mono dim">${e.address}</td>
              </tr>`)}</tbody>
            </table></div>`}
      </section>
    `}renderLogs(){let e=this.logFilter,t=[[`all`,`All`],[`2xx`,`2xx`],[`3xx`,`3xx`],[`4xx`,`4xx`],[`5xx`,`5xx`]],n=this.paused?this.logFrozen:this.logAll,r=e.text||e.status!==`all`,i=r?n.filter(e=>this.logShown(e)):n,a=i.slice(-300).reverse(),o=this.paused?this.logAll.length-this.logFrozen.length:0;return h`
      ${this.head(`Logs`,`One request per row, newest first. This console's own requests aren't logged.`)}
      <div class="row log-tools">
        <lit-material-text-field class="log-filter" variant="outlined" label="Filter by path, host, address…" .value=${e.text} @input=${e=>this.setLogFilter({text:e.target.value})}></lit-material-text-field>
        <div class="seg" role="group" aria-label="Status">
          ${t.map(([t,n])=>h`<button aria-pressed=${e.status===t} @click=${()=>this.setLogFilter({status:t})}>${n}</button>`)}
        </div>
        <span class="spacer"></span>
        <button class="btn" aria-pressed=${!this.paused} @click=${()=>this.setPaused(!this.paused)}>
          <span class="live" data-on=${!this.paused}><span class="dot" aria-hidden="true"></span>${this.paused?`Paused`:`Following`}</span></button>
        <button class="btn" ?disabled=${i.length===0} @click=${()=>this.downloadLog()}><span class="icon">${Z(`download-simple`)}</span>Download</button>
      </div>
      <section class="card" aria-label="Access log">
        <div class="reqs" role="table" aria-label="Access log">
          ${wa}
          ${a.length===0?h`<div class="req-none">${r&&n.length?`No request matches the filter.`:`No requests yet. They appear here as they arrive.`}</div>`:a.map(Ca)}
        </div>
        <div class="card-foot">${this.paused?h`Paused${o>0?h`: ${$(o,`new request`,`new requests`)} waiting.`:`.`} Follow again to see new requests.`:h`Showing ${a.length.toLocaleString()}${i.length>a.length?h` of ${i.length.toLocaleString()}`:v}${r?` that match`:``}. The last ${oa.toLocaleString()} are kept here; Download saves them.`}</div>
      </section>
    `}};customElements.define(`rws-admin`,Da);