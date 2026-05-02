import{a as e,i as t,n,o as r,r as i,t as a}from"./chunk-CFjPhJqf.js";import{l as o,u as s}from"./lit-DvAwKlSm.js";var c=`modulepreload`,l=function(e,t){return new URL(e,t).href},u={},d=function(e,t,n){let r=Promise.resolve();if(t&&t.length>0){let e=document.getElementsByTagName(`link`),i=document.querySelector(`meta[property=csp-nonce]`),a=i?.nonce||i?.getAttribute(`nonce`);function o(e){return Promise.all(e.map(e=>Promise.resolve(e).then(e=>({status:`fulfilled`,value:e}),e=>({status:`rejected`,reason:e}))))}r=o(t.map(t=>{if(t=l(t,n),t in u)return;u[t]=!0;let r=t.endsWith(`.css`),i=r?`[rel="stylesheet"]`:``;if(n)for(let n=e.length-1;n>=0;n--){let i=e[n];if(i.href===t&&(!r||i.rel===`stylesheet`))return}else if(document.querySelector(`link[href="${t}"]${i}`))return;let o=document.createElement(`link`);if(o.rel=r?`stylesheet`:c,r||(o.as=`script`),o.crossOrigin=``,o.href=t,a&&o.setAttribute(`nonce`,a),document.head.appendChild(o),r)return new Promise((e,n)=>{o.addEventListener(`load`,e),o.addEventListener(`error`,()=>n(Error(`Unable to preload CSS for ${t}`)))})}))}function i(e){let t=new Event(`vite:preloadError`,{cancelable:!0});if(t.payload=e,window.dispatchEvent(t),!t.defaultPrevented)throw e}return r.then(t=>{for(let e of t||[])e.status===`rejected`&&i(e.reason);return e().catch(i)})},f=a(((e,t)=>{var n=typeof Reflect==`object`?Reflect:null,r=n&&typeof n.apply==`function`?n.apply:function(e,t,n){return Function.prototype.apply.call(e,t,n)},i=n&&typeof n.ownKeys==`function`?n.ownKeys:Object.getOwnPropertySymbols?function(e){return Object.getOwnPropertyNames(e).concat(Object.getOwnPropertySymbols(e))}:function(e){return Object.getOwnPropertyNames(e)};function a(e){console&&console.warn&&console.warn(e)}var o=Number.isNaN||function(e){return e!==e};function s(){s.init.call(this)}t.exports=s,t.exports.once=y,s.EventEmitter=s,s.prototype._events=void 0,s.prototype._eventsCount=0,s.prototype._maxListeners=void 0;var c=10;function l(e){if(typeof e!=`function`)throw TypeError(`The "listener" argument must be of type Function. Received type `+typeof e)}Object.defineProperty(s,`defaultMaxListeners`,{enumerable:!0,get:function(){return c},set:function(e){if(typeof e!=`number`||e<0||o(e))throw RangeError(`The value of "defaultMaxListeners" is out of range. It must be a non-negative number. Received `+e+`.`);c=e}}),s.init=function(){(this._events===void 0||this._events===Object.getPrototypeOf(this)._events)&&(this._events=Object.create(null),this._eventsCount=0),this._maxListeners=this._maxListeners||void 0},s.prototype.setMaxListeners=function(e){if(typeof e!=`number`||e<0||o(e))throw RangeError(`The value of "n" is out of range. It must be a non-negative number. Received `+e+`.`);return this._maxListeners=e,this};function u(e){return e._maxListeners===void 0?s.defaultMaxListeners:e._maxListeners}s.prototype.getMaxListeners=function(){return u(this)},s.prototype.emit=function(e){for(var t=[],n=1;n<arguments.length;n++)t.push(arguments[n]);var i=e===`error`,a=this._events;if(a!==void 0)i&&=a.error===void 0;else if(!i)return!1;if(i){var o;if(t.length>0&&(o=t[0]),o instanceof Error)throw o;var s=Error(`Unhandled error.`+(o?` (`+o.message+`)`:``));throw s.context=o,s}var c=a[e];if(c===void 0)return!1;if(typeof c==`function`)r(c,this,t);else for(var l=c.length,u=g(c,l),n=0;n<l;++n)r(u[n],this,t);return!0};function d(e,t,n,r){var i,o,s;if(l(n),o=e._events,o===void 0?(o=e._events=Object.create(null),e._eventsCount=0):(o.newListener!==void 0&&(e.emit(`newListener`,t,n.listener?n.listener:n),o=e._events),s=o[t]),s===void 0)s=o[t]=n,++e._eventsCount;else if(typeof s==`function`?s=o[t]=r?[n,s]:[s,n]:r?s.unshift(n):s.push(n),i=u(e),i>0&&s.length>i&&!s.warned){s.warned=!0;var c=Error(`Possible EventEmitter memory leak detected. `+s.length+` `+String(t)+` listeners added. Use emitter.setMaxListeners() to increase limit`);c.name=`MaxListenersExceededWarning`,c.emitter=e,c.type=t,c.count=s.length,a(c)}return e}s.prototype.addListener=function(e,t){return d(this,e,t,!1)},s.prototype.on=s.prototype.addListener,s.prototype.prependListener=function(e,t){return d(this,e,t,!0)};function f(){if(!this.fired)return this.target.removeListener(this.type,this.wrapFn),this.fired=!0,arguments.length===0?this.listener.call(this.target):this.listener.apply(this.target,arguments)}function p(e,t,n){var r={fired:!1,wrapFn:void 0,target:e,type:t,listener:n},i=f.bind(r);return i.listener=n,r.wrapFn=i,i}s.prototype.once=function(e,t){return l(t),this.on(e,p(this,e,t)),this},s.prototype.prependOnceListener=function(e,t){return l(t),this.prependListener(e,p(this,e,t)),this},s.prototype.removeListener=function(e,t){var n,r,i,a,o;if(l(t),r=this._events,r===void 0||(n=r[e],n===void 0))return this;if(n===t||n.listener===t)--this._eventsCount===0?this._events=Object.create(null):(delete r[e],r.removeListener&&this.emit(`removeListener`,e,n.listener||t));else if(typeof n!=`function`){for(i=-1,a=n.length-1;a>=0;a--)if(n[a]===t||n[a].listener===t){o=n[a].listener,i=a;break}if(i<0)return this;i===0?n.shift():_(n,i),n.length===1&&(r[e]=n[0]),r.removeListener!==void 0&&this.emit(`removeListener`,e,o||t)}return this},s.prototype.off=s.prototype.removeListener,s.prototype.removeAllListeners=function(e){var t,n=this._events,r;if(n===void 0)return this;if(n.removeListener===void 0)return arguments.length===0?(this._events=Object.create(null),this._eventsCount=0):n[e]!==void 0&&(--this._eventsCount===0?this._events=Object.create(null):delete n[e]),this;if(arguments.length===0){var i=Object.keys(n),a;for(r=0;r<i.length;++r)a=i[r],a!==`removeListener`&&this.removeAllListeners(a);return this.removeAllListeners(`removeListener`),this._events=Object.create(null),this._eventsCount=0,this}if(t=n[e],typeof t==`function`)this.removeListener(e,t);else if(t!==void 0)for(r=t.length-1;r>=0;r--)this.removeListener(e,t[r]);return this};function m(e,t,n){var r=e._events;if(r===void 0)return[];var i=r[t];return i===void 0?[]:typeof i==`function`?n?[i.listener||i]:[i]:n?v(i):g(i,i.length)}s.prototype.listeners=function(e){return m(this,e,!0)},s.prototype.rawListeners=function(e){return m(this,e,!1)},s.listenerCount=function(e,t){return typeof e.listenerCount==`function`?e.listenerCount(t):h.call(e,t)},s.prototype.listenerCount=h;function h(e){var t=this._events;if(t!==void 0){var n=t[e];if(typeof n==`function`)return 1;if(n!==void 0)return n.length}return 0}s.prototype.eventNames=function(){return this._eventsCount>0?i(this._events):[]};function g(e,t){for(var n=Array(t),r=0;r<t;++r)n[r]=e[r];return n}function _(e,t){for(;t+1<e.length;t++)e[t]=e[t+1];e.pop()}function v(e){for(var t=Array(e.length),n=0;n<t.length;++n)t[n]=e[n].listener||e[n];return t}function y(e,t){return new Promise(function(n,r){function i(n){e.removeListener(t,a),r(n)}function a(){typeof e.removeListener==`function`&&e.removeListener(`error`,i),n([].slice.call(arguments))}ee(e,t,a,{once:!0}),t!==`error`&&b(e,i,{once:!0})})}function b(e,t,n){typeof e.on==`function`&&ee(e,`error`,t,n)}function ee(e,t,n,r){if(typeof e.on==`function`)r.once?e.once(t,n):e.on(t,n);else if(typeof e.addEventListener==`function`)e.addEventListener(t,function i(a){r.once&&e.removeEventListener(t,i),n(a)});else throw TypeError(`The "emitter" argument must be of type EventEmitter. Received type `+typeof e)}})),p=e=>JSON.stringify(e,(e,t)=>typeof t==`bigint`?t.toString()+`n`:t),m=e=>{let t=e.replace(/([\[:])?(\d{17,}|(?:[9](?:[1-9]07199254740991|0[1-9]7199254740991|00[8-9]199254740991|007[2-9]99254740991|007199[3-9]54740991|0071992[6-9]4740991|00719925[5-9]740991|007199254[8-9]40991|0071992547[5-9]0991|00719925474[1-9]991|00719925474099[2-9])))([,\}\]])/g,`$1"$2n"$3`);return JSON.parse(t,(e,t)=>typeof t==`string`&&t.match(/^\d+n$/)?BigInt(t.substring(0,t.length-1)):t)};function h(e){if(typeof e!=`string`)throw Error(`Cannot safe json parse value of type ${typeof e}`);try{return m(e)}catch{return e}}function g(e){return typeof e==`string`?e:p(e)||``}var _=a(((e,t)=>{function n(e){try{return JSON.stringify(e)}catch{return`"[Circular]"`}}t.exports=r;function r(e,t,r){var i=r&&r.stringify||n,a=1;if(typeof e==`object`&&e){var o=t.length+a;if(o===1)return e;var s=Array(o);s[0]=i(e);for(var c=1;c<o;c++)s[c]=i(t[c]);return s.join(` `)}if(typeof e!=`string`)return e;var l=t.length;if(l===0)return e;for(var u=``,d=1-a,f=-1,p=e&&e.length||0,m=0;m<p;){if(e.charCodeAt(m)===37&&m+1<p){switch(f=f>-1?f:0,e.charCodeAt(m+1)){case 100:case 102:if(d>=l||t[d]==null)break;f<m&&(u+=e.slice(f,m)),u+=Number(t[d]),f=m+2,m++;break;case 105:if(d>=l||t[d]==null)break;f<m&&(u+=e.slice(f,m)),u+=Math.floor(Number(t[d])),f=m+2,m++;break;case 79:case 111:case 106:if(d>=l||t[d]===void 0)break;f<m&&(u+=e.slice(f,m));var h=typeof t[d];if(h===`string`){u+=`'`+t[d]+`'`,f=m+2,m++;break}if(h===`function`){u+=t[d].name||`<anonymous>`,f=m+2,m++;break}u+=i(t[d]),f=m+2,m++;break;case 115:if(d>=l)break;f<m&&(u+=e.slice(f,m)),u+=String(t[d]),f=m+2,m++;break;case 37:f<m&&(u+=e.slice(f,m)),u+=`%`,f=m+2,m++,d--;break}++d}++m}return f===-1?e:(f<p&&(u+=e.slice(f)),u)}})),v=r(a(((e,t)=>{var n=_();t.exports=o;var r=ne().console||{},i={mapHttpRequest:g,mapHttpResponse:g,wrapRequestSerializer:v,wrapResponseSerializer:v,wrapErrorSerializer:v,req:g,res:g,err:m};function a(e,t){return Array.isArray(e)?e.filter(function(e){return e!==`!stdSerializers.err`}):e===!0?Object.keys(t):!1}function o(e){e||={},e.browser=e.browser||{};let t=e.browser.transmit;if(t&&typeof t.send!=`function`)throw Error(`pino: transmit option must have a send function`);let n=e.browser.write||r;e.browser.write&&(e.browser.asObject=!0);let i=e.serializers||{},c=a(e.browser.serialize,i),l=e.browser.serialize;Array.isArray(e.browser.serialize)&&e.browser.serialize.indexOf(`!stdSerializers.err`)>-1&&(l=!1);let f=[`error`,`fatal`,`warn`,`info`,`debug`,`trace`];typeof n==`function`&&(n.error=n.fatal=n.warn=n.info=n.debug=n.trace=n),e.enabled===!1&&(e.level=`silent`);let m=e.level||`info`,g=Object.create(n);g.log||=y,Object.defineProperty(g,`levelVal`,{get:v}),Object.defineProperty(g,`level`,{get:b,set:ee});let _={transmit:t,serialize:c,asObject:e.browser.asObject,levels:f,timestamp:h(e)};g.levels=o.levels,g.level=m,g.setMaxListeners=g.getMaxListeners=g.emit=g.addListener=g.on=g.prependListener=g.once=g.prependOnceListener=g.removeListener=g.removeAllListeners=g.listeners=g.listenerCount=g.eventNames=g.write=g.flush=y,g.serializers=i,g._serialize=c,g._stdErrSerialize=l,g.child=te,t&&(g._logEvent=p());function v(){return this.level===`silent`?1/0:this.levels.values[this.level]}function b(){return this._level}function ee(e){if(e!==`silent`&&!this.levels.values[e])throw Error(`unknown level `+e);this._level=e,s(_,g,`error`,`log`),s(_,g,`fatal`,`error`),s(_,g,`warn`,`error`),s(_,g,`info`,`log`),s(_,g,`debug`,`log`),s(_,g,`trace`,`log`)}function te(n,r){if(!n)throw Error(`missing bindings for child Pino`);r||={},c&&n.serializers&&(r.serializers=n.serializers);let a=r.serializers;if(c&&a){var o=Object.assign({},i,a),s=e.browser.serialize===!0?Object.keys(o):c;delete n.serializers,u([n],s,o,this._stdErrSerialize)}function l(e){this._childLevel=(e._childLevel|0)+1,this.error=d(e,n,`error`),this.fatal=d(e,n,`fatal`),this.warn=d(e,n,`warn`),this.info=d(e,n,`info`),this.debug=d(e,n,`debug`),this.trace=d(e,n,`trace`),o&&(this.serializers=o,this._serialize=s),t&&(this._logEvent=p([].concat(e._logEvent.bindings,n)))}return l.prototype=this,new l(this)}return g}o.levels={values:{fatal:60,error:50,warn:40,info:30,debug:20,trace:10},labels:{10:`trace`,20:`debug`,30:`info`,40:`warn`,50:`error`,60:`fatal`}},o.stdSerializers=i,o.stdTimeFunctions=Object.assign({},{nullTime:b,epochTime:ee,unixTime:te,isoTime:x});function s(e,t,n,i){let a=Object.getPrototypeOf(t);t[n]=t.levelVal>t.levels.values[n]?y:a[n]?a[n]:r[n]||r[i]||y,c(e,t,n)}function c(e,t,n){!e.transmit&&t[n]===y||(t[n]=(function(i){return function(){let a=e.timestamp(),s=Array(arguments.length),c=Object.getPrototypeOf&&Object.getPrototypeOf(this)===r?r:this;for(var d=0;d<s.length;d++)s[d]=arguments[d];if(e.serialize&&!e.asObject&&u(s,this._serialize,this.serializers,this._stdErrSerialize),e.asObject?i.call(c,l(this,n,s,a)):i.apply(c,s),e.transmit){let r=e.transmit.level||t.level,i=o.levels.values[r],c=o.levels.values[n];if(c<i)return;f(this,{ts:a,methodLevel:n,methodValue:c,transmitLevel:r,transmitValue:o.levels.values[e.transmit.level||t.level],send:e.transmit.send,val:t.levelVal},s)}}})(t[n]))}function l(e,t,r,i){e._serialize&&u(r,e._serialize,e.serializers,e._stdErrSerialize);let a=r.slice(),s=a[0],c={};i&&(c.time=i),c.level=o.levels.values[t];let l=(e._childLevel|0)+1;if(l<1&&(l=1),typeof s==`object`&&s){for(;l--&&typeof a[0]==`object`;)Object.assign(c,a.shift());s=a.length?n(a.shift(),a):void 0}else typeof s==`string`&&(s=n(a.shift(),a));return s!==void 0&&(c.msg=s),c}function u(e,t,n,r){for(let i in e)if(r&&e[i]instanceof Error)e[i]=o.stdSerializers.err(e[i]);else if(typeof e[i]==`object`&&!Array.isArray(e[i]))for(let r in e[i])t&&t.indexOf(r)>-1&&r in n&&(e[i][r]=n[r](e[i][r]))}function d(e,t,n){return function(){let r=Array(1+arguments.length);r[0]=t;for(var i=1;i<r.length;i++)r[i]=arguments[i-1];return e[n].apply(this,r)}}function f(e,t,n){let r=t.send,i=t.ts,a=t.methodLevel,o=t.methodValue,s=t.val,c=e._logEvent.bindings;u(n,e._serialize||Object.keys(e.serializers),e.serializers,e._stdErrSerialize===void 0?!0:e._stdErrSerialize),e._logEvent.ts=i,e._logEvent.messages=n.filter(function(e){return c.indexOf(e)===-1}),e._logEvent.level.label=a,e._logEvent.level.value=o,r(a,e._logEvent,s),e._logEvent=p(c)}function p(e){return{ts:0,messages:[],bindings:e||[],level:{label:``,value:0}}}function m(e){let t={type:e.constructor.name,msg:e.message,stack:e.stack};for(let n in e)t[n]===void 0&&(t[n]=e[n]);return t}function h(e){return typeof e.timestamp==`function`?e.timestamp:e.timestamp===!1?b:ee}function g(){return{}}function v(e){return e}function y(){}function b(){return!1}function ee(){return Date.now()}function te(){return Math.round(Date.now()/1e3)}function x(){return new Date(Date.now()).toISOString()}function ne(){function e(e){return e!==void 0&&e}try{return typeof globalThis<`u`||Object.defineProperty(Object.prototype,`globalThis`,{get:function(){return delete Object.prototype.globalThis,this.globalThis=this},configurable:!0}),globalThis}catch{return e(self)||e(window)||e(this)||{}}}}))()),y={level:`info`},b=`custom_context`,ee=1e3*1024,te=class{constructor(e){this.nodeValue=e,this.sizeInBytes=new TextEncoder().encode(this.nodeValue).length,this.next=null}get value(){return this.nodeValue}get size(){return this.sizeInBytes}},x=class{constructor(e){this.head=null,this.tail=null,this.lengthInNodes=0,this.maxSizeInBytes=e,this.sizeInBytes=0}append(e){let t=new te(e);if(t.size>this.maxSizeInBytes)throw Error(`[LinkedList] Value too big to insert into list: ${e} with size ${t.size}`);for(;this.size+t.size>this.maxSizeInBytes;)this.shift();this.head?(this.tail&&(this.tail.next=t),this.tail=t):(this.head=t,this.tail=t),this.lengthInNodes++,this.sizeInBytes+=t.size}shift(){if(!this.head)return;let e=this.head;this.head=this.head.next,this.head||(this.tail=null),this.lengthInNodes--,this.sizeInBytes-=e.size}toArray(){let e=[],t=this.head;for(;t!==null;)e.push(t.value),t=t.next;return e}get length(){return this.lengthInNodes}get size(){return this.sizeInBytes}toOrderedArray(){return Array.from(this)}[Symbol.iterator](){let e=this.head;return{next:()=>{if(!e)return{done:!0,value:null};let t=e.value;return e=e.next,{done:!1,value:t}}}}},ne=class{constructor(e,t=ee){this.level=e??`error`,this.levelValue=v.levels.values[this.level],this.MAX_LOG_SIZE_IN_BYTES=t,this.logs=new x(this.MAX_LOG_SIZE_IN_BYTES)}forwardToConsole(e,t){t===v.levels.values.error?console.error(e):t===v.levels.values.warn?console.warn(e):t===v.levels.values.debug?console.debug(e):t===v.levels.values.trace?console.trace(e):console.log(e)}appendToLogs(e){this.logs.append(g({timestamp:new Date().toISOString(),log:e}));let t=typeof e==`string`?JSON.parse(e).level:e.level;t>=this.levelValue&&this.forwardToConsole(e,t)}getLogs(){return this.logs}clearLogs(){this.logs=new x(this.MAX_LOG_SIZE_IN_BYTES)}getLogArray(){return Array.from(this.logs)}logsToBlob(e){let t=this.getLogArray();return t.push(g({extraMetadata:e})),new Blob(t,{type:`application/json`})}},re=class{constructor(e,t=ee){this.baseChunkLogger=new ne(e,t)}write(e){this.baseChunkLogger.appendToLogs(e)}getLogs(){return this.baseChunkLogger.getLogs()}clearLogs(){this.baseChunkLogger.clearLogs()}getLogArray(){return this.baseChunkLogger.getLogArray()}logsToBlob(e){return this.baseChunkLogger.logsToBlob(e)}downloadLogsBlobInBrowser(e){let t=URL.createObjectURL(this.logsToBlob(e)),n=document.createElement(`a`);n.href=t,n.download=`walletconnect-logs-${new Date().toISOString()}.txt`,document.body.appendChild(n),n.click(),document.body.removeChild(n),URL.revokeObjectURL(t)}},ie=class{constructor(e,t=ee){this.baseChunkLogger=new ne(e,t)}write(e){this.baseChunkLogger.appendToLogs(e)}getLogs(){return this.baseChunkLogger.getLogs()}clearLogs(){this.baseChunkLogger.clearLogs()}getLogArray(){return this.baseChunkLogger.getLogArray()}logsToBlob(e){return this.baseChunkLogger.logsToBlob(e)}},ae=Object.defineProperty,oe=Object.defineProperties,se=Object.getOwnPropertyDescriptors,ce=Object.getOwnPropertySymbols,le=Object.prototype.hasOwnProperty,ue=Object.prototype.propertyIsEnumerable,de=(e,t,n)=>t in e?ae(e,t,{enumerable:!0,configurable:!0,writable:!0,value:n}):e[t]=n,fe=(e,t)=>{for(var n in t||={})le.call(t,n)&&de(e,n,t[n]);if(ce)for(var n of ce(t))ue.call(t,n)&&de(e,n,t[n]);return e},pe=(e,t)=>oe(e,se(t));function me(e){return pe(fe({},e),{level:e?.level||y.level})}function he(e,t=b){return e[t]||``}function ge(e,t,n=b){return e[n]=t,e}function _e(e,t=b){let n=``;return n=typeof e.bindings>`u`?he(e,t):e.bindings().context||``,n}function ve(e,t,n=b){let r=_e(e,n);return r.trim()?`${r}/${t}`:t}function ye(e,t,n=b){let r=ve(e,t,n);return ge(e.child({context:r}),r,n)}function be(e){let t=new re(e.opts?.level,e.maxSizeInBytes);return{logger:(0,v.default)(pe(fe({},e.opts),{level:`trace`,browser:pe(fe({},e.opts?.browser),{write:e=>t.write(e)})})),chunkLoggerController:t}}function xe(e){let t=new ie(e.opts?.level,e.maxSizeInBytes);return{logger:(0,v.default)(pe(fe({},e.opts),{level:`trace`}),t),chunkLoggerController:t}}function Se(e){return typeof e.loggerOverride<`u`&&typeof e.loggerOverride!=`string`?{logger:e.loggerOverride,chunkLoggerController:null}:typeof window<`u`?be(e):xe(e)}var Ce=`PARSE_ERROR`,we=`INVALID_REQUEST`,Te=`METHOD_NOT_FOUND`,Ee=`INVALID_PARAMS`,De=`INTERNAL_ERROR`,Oe=`SERVER_ERROR`,ke=[-32700,-32600,-32601,-32602,-32603],Ae=[-32e3,-32099],je={[Ce]:{code:-32700,message:`Parse error`},[we]:{code:-32600,message:`Invalid Request`},[Te]:{code:-32601,message:`Method not found`},[Ee]:{code:-32602,message:`Invalid params`},[De]:{code:-32603,message:`Internal error`},[Oe]:{code:-32e3,message:`Server error`}},Me=Oe;function Ne(e){return e<=Ae[0]&&e>=Ae[1]}function Pe(e){return ke.includes(e)}function Fe(e){return typeof e==`number`}function Ie(e){return Object.keys(je).includes(e)?je[e]:je[Me]}function Le(e){return Object.values(je).find(t=>t.code===e)||je[Me]}function Re(e){if(e.error.code===void 0)return{valid:!1,error:`Missing code for JSON-RPC error`};if(e.error.message===void 0)return{valid:!1,error:`Missing message for JSON-RPC error`};if(!Fe(e.error.code))return{valid:!1,error:`Invalid error code type for JSON-RPC: ${e.error.code}`};if(Pe(e.error.code)){let t=Le(e.error.code);if(t.message!==je.SERVER_ERROR.message&&e.error.message===t.message)return{valid:!1,error:`Invalid error code message for JSON-RPC: ${e.error.code}`}}return{valid:!0}}function ze(e,t,n){return e.message.includes(`getaddrinfo ENOTFOUND`)||e.message.includes(`connect ECONNREFUSED`)?Error(`Unavailable ${n} RPC url at ${t}`):e}var Be=i({__assign:()=>ut,__asyncDelegator:()=>nt,__asyncGenerator:()=>tt,__asyncValues:()=>rt,__await:()=>et,__awaiter:()=>Ke,__classPrivateFieldGet:()=>st,__classPrivateFieldSet:()=>ct,__createBinding:()=>Je,__decorate:()=>Ue,__exportStar:()=>Ye,__extends:()=>Ve,__generator:()=>qe,__importDefault:()=>ot,__importStar:()=>at,__makeTemplateObject:()=>it,__metadata:()=>Ge,__param:()=>We,__read:()=>Ze,__rest:()=>He,__spread:()=>Qe,__spreadArrays:()=>$e,__values:()=>Xe});function Ve(e,t){lt(e,t);function n(){this.constructor=e}e.prototype=t===null?Object.create(t):(n.prototype=t.prototype,new n)}function He(e,t){var n={};for(var r in e)Object.prototype.hasOwnProperty.call(e,r)&&t.indexOf(r)<0&&(n[r]=e[r]);if(e!=null&&typeof Object.getOwnPropertySymbols==`function`)for(var i=0,r=Object.getOwnPropertySymbols(e);i<r.length;i++)t.indexOf(r[i])<0&&Object.prototype.propertyIsEnumerable.call(e,r[i])&&(n[r[i]]=e[r[i]]);return n}function Ue(e,t,n,r){var i=arguments.length,a=i<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,n):r,o;if(typeof Reflect==`object`&&typeof Reflect.decorate==`function`)a=Reflect.decorate(e,t,n,r);else for(var s=e.length-1;s>=0;s--)(o=e[s])&&(a=(i<3?o(a):i>3?o(t,n,a):o(t,n))||a);return i>3&&a&&Object.defineProperty(t,n,a),a}function We(e,t){return function(n,r){t(n,r,e)}}function Ge(e,t){if(typeof Reflect==`object`&&typeof Reflect.metadata==`function`)return Reflect.metadata(e,t)}function Ke(e,t,n,r){function i(e){return e instanceof n?e:new n(function(t){t(e)})}return new(n||=Promise)(function(n,a){function o(e){try{c(r.next(e))}catch(e){a(e)}}function s(e){try{c(r.throw(e))}catch(e){a(e)}}function c(e){e.done?n(e.value):i(e.value).then(o,s)}c((r=r.apply(e,t||[])).next())})}function qe(e,t){var n={label:0,sent:function(){if(a[0]&1)throw a[1];return a[1]},trys:[],ops:[]},r,i,a,o;return o={next:s(0),throw:s(1),return:s(2)},typeof Symbol==`function`&&(o[Symbol.iterator]=function(){return this}),o;function s(e){return function(t){return c([e,t])}}function c(o){if(r)throw TypeError(`Generator is already executing.`);for(;n;)try{if(r=1,i&&(a=o[0]&2?i.return:o[0]?i.throw||((a=i.return)&&a.call(i),0):i.next)&&!(a=a.call(i,o[1])).done)return a;switch(i=0,a&&(o=[o[0]&2,a.value]),o[0]){case 0:case 1:a=o;break;case 4:return n.label++,{value:o[1],done:!1};case 5:n.label++,i=o[1],o=[0];continue;case 7:o=n.ops.pop(),n.trys.pop();continue;default:if((a=n.trys,!(a=a.length>0&&a[a.length-1]))&&(o[0]===6||o[0]===2)){n=0;continue}if(o[0]===3&&(!a||o[1]>a[0]&&o[1]<a[3])){n.label=o[1];break}if(o[0]===6&&n.label<a[1]){n.label=a[1],a=o;break}if(a&&n.label<a[2]){n.label=a[2],n.ops.push(o);break}a[2]&&n.ops.pop(),n.trys.pop();continue}o=t.call(e,n)}catch(e){o=[6,e],i=0}finally{r=a=0}if(o[0]&5)throw o[1];return{value:o[0]?o[1]:void 0,done:!0}}}function Je(e,t,n,r){r===void 0&&(r=n),e[r]=t[n]}function Ye(e,t){for(var n in e)n!==`default`&&!t.hasOwnProperty(n)&&(t[n]=e[n])}function Xe(e){var t=typeof Symbol==`function`&&Symbol.iterator,n=t&&e[t],r=0;if(n)return n.call(e);if(e&&typeof e.length==`number`)return{next:function(){return e&&r>=e.length&&(e=void 0),{value:e&&e[r++],done:!e}}};throw TypeError(t?`Object is not iterable.`:`Symbol.iterator is not defined.`)}function Ze(e,t){var n=typeof Symbol==`function`&&e[Symbol.iterator];if(!n)return e;var r=n.call(e),i,a=[],o;try{for(;(t===void 0||t-- >0)&&!(i=r.next()).done;)a.push(i.value)}catch(e){o={error:e}}finally{try{i&&!i.done&&(n=r.return)&&n.call(r)}finally{if(o)throw o.error}}return a}function Qe(){for(var e=[],t=0;t<arguments.length;t++)e=e.concat(Ze(arguments[t]));return e}function $e(){for(var e=0,t=0,n=arguments.length;t<n;t++)e+=arguments[t].length;for(var r=Array(e),i=0,t=0;t<n;t++)for(var a=arguments[t],o=0,s=a.length;o<s;o++,i++)r[i]=a[o];return r}function et(e){return this instanceof et?(this.v=e,this):new et(e)}function tt(e,t,n){if(!Symbol.asyncIterator)throw TypeError(`Symbol.asyncIterator is not defined.`);var r=n.apply(e,t||[]),i,a=[];return i={},o(`next`),o(`throw`),o(`return`),i[Symbol.asyncIterator]=function(){return this},i;function o(e){r[e]&&(i[e]=function(t){return new Promise(function(n,r){a.push([e,t,n,r])>1||s(e,t)})})}function s(e,t){try{c(r[e](t))}catch(e){d(a[0][3],e)}}function c(e){e.value instanceof et?Promise.resolve(e.value.v).then(l,u):d(a[0][2],e)}function l(e){s(`next`,e)}function u(e){s(`throw`,e)}function d(e,t){e(t),a.shift(),a.length&&s(a[0][0],a[0][1])}}function nt(e){var t,n;return t={},r(`next`),r(`throw`,function(e){throw e}),r(`return`),t[Symbol.iterator]=function(){return this},t;function r(r,i){t[r]=e[r]?function(t){return(n=!n)?{value:et(e[r](t)),done:r===`return`}:i?i(t):t}:i}}function rt(e){if(!Symbol.asyncIterator)throw TypeError(`Symbol.asyncIterator is not defined.`);var t=e[Symbol.asyncIterator],n;return t?t.call(e):(e=typeof Xe==`function`?Xe(e):e[Symbol.iterator](),n={},r(`next`),r(`throw`),r(`return`),n[Symbol.asyncIterator]=function(){return this},n);function r(t){n[t]=e[t]&&function(n){return new Promise(function(r,a){n=e[t](n),i(r,a,n.done,n.value)})}}function i(e,t,n,r){Promise.resolve(r).then(function(t){e({value:t,done:n})},t)}}function it(e,t){return Object.defineProperty?Object.defineProperty(e,`raw`,{value:t}):e.raw=t,e}function at(e){if(e&&e.__esModule)return e;var t={};if(e!=null)for(var n in e)Object.hasOwnProperty.call(e,n)&&(t[n]=e[n]);return t.default=e,t}function ot(e){return e&&e.__esModule?e:{default:e}}function st(e,t){if(!t.has(e))throw TypeError(`attempted to get private field on non-instance`);return t.get(e)}function ct(e,t,n){if(!t.has(e))throw TypeError(`attempted to set private field on non-instance`);return t.set(e,n),n}var lt,ut,dt=n((()=>{lt=function(e,t){return lt=Object.setPrototypeOf||{__proto__:[]}instanceof Array&&function(e,t){e.__proto__=t}||function(e,t){for(var n in t)t.hasOwnProperty(n)&&(e[n]=t[n])},lt(e,t)},ut=function(){return ut=Object.assign||function(e){for(var t,n=1,r=arguments.length;n<r;n++)for(var i in t=arguments[n],t)Object.prototype.hasOwnProperty.call(t,i)&&(e[i]=t[i]);return e},ut.apply(this,arguments)}})),ft=a((e=>{Object.defineProperty(e,`__esModule`,{value:!0}),e.isBrowserCryptoAvailable=e.getSubtleCrypto=e.getBrowerCrypto=void 0;function t(){return(global==null?void 0:global.crypto)||(global==null?void 0:global.msCrypto)||{}}e.getBrowerCrypto=t;function n(){let e=t();return e.subtle||e.webkitSubtle}e.getSubtleCrypto=n;function r(){return!!t()&&!!n()}e.isBrowserCryptoAvailable=r})),pt=a((e=>{Object.defineProperty(e,`__esModule`,{value:!0}),e.isBrowser=e.isNode=e.isReactNative=void 0;function t(){return typeof document>`u`&&typeof navigator<`u`&&navigator.product===`ReactNative`}e.isReactNative=t;function n(){return typeof process<`u`&&process.versions!==void 0&&process.versions.node!==void 0}e.isNode=n;function r(){return!t()&&!n()}e.isBrowser=r})),mt=a((t=>{Object.defineProperty(t,`__esModule`,{value:!0});var n=(dt(),e(Be));n.__exportStar(ft(),t),n.__exportStar(pt(),t)})),ht=i({isNodeJs:()=>_t}),gt=mt();t(ht,r(mt()));var _t=gt.isNode;function vt(e=3){return Date.now()*10**e+Math.floor(Math.random()*10**e)}function yt(e=6){return BigInt(vt(e))}function bt(e,t,n){return{id:n||vt(),jsonrpc:`2.0`,method:e,params:t}}function xt(e,t){return{id:e,jsonrpc:`2.0`,result:t}}function St(e,t,n){return{id:e,jsonrpc:`2.0`,error:Ct(t,n)}}function Ct(e,t){return e===void 0?Ie(De):(typeof e==`string`&&(e=Object.assign(Object.assign({},Ie(Oe)),{message:e})),t!==void 0&&(e.data=t),Pe(e.code)&&(e=Le(e.code)),e)}function wt(e){return e.includes(`*`)?Et(e):!/\W/g.test(e)}function Tt(e){return e===`*`}function Et(e){return Tt(e)?!0:!(!e.includes(`*`)||e.split(`*`).length!==2||e.split(`*`).filter(e=>e.trim()===``).length!==1)}function Dt(e){return!Tt(e)&&Et(e)&&!e.split(`*`)[0].trim()}function Ot(e){return!Tt(e)&&Et(e)&&!e.split(`*`)[1].trim()}var kt=class{},At=class extends kt{constructor(e){super()}},jt=class extends kt{constructor(){super()}},Mt=class extends jt{constructor(e){super()}},Nt=`^https?:`,Pt=`^wss?:`;function Ft(e){let t=e.match(new RegExp(/^\w+:/,`gi`));if(!(!t||!t.length))return t[0]}function It(e,t){let n=Ft(e);return n===void 0?!1:new RegExp(t).test(n)}function Lt(e){return It(e,Nt)}function Rt(e){return It(e,Pt)}function zt(e){return RegExp(`wss?://localhost(:d{2,5})?`).test(e)}function Bt(e){return typeof e==`object`&&`id`in e&&`jsonrpc`in e&&e.jsonrpc===`2.0`}function Vt(e){return Bt(e)&&`method`in e}function Ht(e){return Bt(e)&&(Ut(e)||Wt(e))}function Ut(e){return`result`in e}function Wt(e){return`error`in e}function Gt(e){return`error`in e&&e.valid===!1}var Kt=i({DEFAULT_ERROR:()=>Me,IBaseJsonRpcProvider:()=>jt,IEvents:()=>kt,IJsonRpcConnection:()=>At,IJsonRpcProvider:()=>Mt,INTERNAL_ERROR:()=>De,INVALID_PARAMS:()=>Ee,INVALID_REQUEST:()=>we,METHOD_NOT_FOUND:()=>Te,PARSE_ERROR:()=>Ce,RESERVED_ERROR_CODES:()=>ke,SERVER_ERROR:()=>Oe,SERVER_ERROR_CODE_RANGE:()=>Ae,STANDARD_ERROR_MAP:()=>je,formatErrorMessage:()=>Ct,formatJsonRpcError:()=>St,formatJsonRpcRequest:()=>bt,formatJsonRpcResult:()=>xt,getBigIntRpcId:()=>yt,getError:()=>Ie,getErrorByCode:()=>Le,isHttpUrl:()=>Lt,isJsonRpcError:()=>Wt,isJsonRpcPayload:()=>Bt,isJsonRpcRequest:()=>Vt,isJsonRpcResponse:()=>Ht,isJsonRpcResult:()=>Ut,isJsonRpcValidationInvalid:()=>Gt,isLocalhostUrl:()=>zt,isNodeJs:()=>_t,isReservedErrorCode:()=>Pe,isServerErrorCode:()=>Ne,isValidDefaultRoute:()=>Tt,isValidErrorCode:()=>Fe,isValidLeadingWildcardRoute:()=>Dt,isValidRoute:()=>wt,isValidTrailingWildcardRoute:()=>Ot,isValidWildcardRoute:()=>Et,isWsUrl:()=>Rt,parseConnectionError:()=>ze,payloadId:()=>vt,validateJsonRpcError:()=>Re});t(Kt,ht);var qt=r(f()),Jt=class extends Mt{constructor(e){super(e),this.events=new qt.EventEmitter,this.hasRegisteredEventListeners=!1,this.connection=this.setConnection(e),this.connection.connected&&this.registerEventListeners()}async connect(e=this.connection){await this.open(e)}async disconnect(){await this.close()}on(e,t){this.events.on(e,t)}once(e,t){this.events.once(e,t)}off(e,t){this.events.off(e,t)}removeListener(e,t){this.events.removeListener(e,t)}async request(e,t){return this.requestStrict(bt(e.method,e.params||[],e.id||yt().toString()),t)}async requestStrict(e,t){return new Promise(async(n,r)=>{if(!this.connection.connected)try{await this.open()}catch(e){r(e)}this.events.on(`${e.id}`,e=>{Wt(e)?r(e.error):n(e.result)});try{await this.connection.send(e,t)}catch(e){r(e)}})}setConnection(e=this.connection){return e}onPayload(e){this.events.emit(`payload`,e),Ht(e)?this.events.emit(`${e.id}`,e):this.events.emit(`message`,{type:e.method,data:e.params})}onClose(e){e&&e.code===3e3&&this.events.emit(`error`,Error(`WebSocket connection closed abnormally with code: ${e.code} ${e.reason?`(${e.reason})`:``}`)),this.events.emit(`disconnect`)}async open(e=this.connection){this.connection===e&&this.connection.connected||(this.connection.connected&&this.close(),typeof e==`string`&&(await this.connection.open(e),e=this.connection),this.connection=this.setConnection(e),await this.connection.open(),this.registerEventListeners(),this.events.emit(`connect`))}async close(){await this.connection.close()}registerEventListeners(){this.hasRegisteredEventListeners||=(this.connection.on(`payload`,e=>this.onPayload(e)),this.connection.on(`close`,e=>this.onClose(e)),this.connection.on(`error`,e=>this.events.emit(`error`,e)),this.connection.on(`register_error`,e=>this.onClose()),!0)}},Yt=r(a(((e,t)=>{var n=typeof globalThis<`u`&&globalThis||typeof self<`u`&&self||typeof global<`u`&&global,r=(function(){function e(){this.fetch=!1,this.DOMException=n.DOMException}return e.prototype=n,new e})();(function(e){(function(t){var n=e!==void 0&&e||typeof self<`u`&&self||typeof global<`u`&&global||{},r={searchParams:`URLSearchParams`in n,iterable:`Symbol`in n&&`iterator`in Symbol,blob:`FileReader`in n&&`Blob`in n&&(function(){try{return new Blob,!0}catch{return!1}})(),formData:`FormData`in n,arrayBuffer:`ArrayBuffer`in n};function i(e){return e&&DataView.prototype.isPrototypeOf(e)}if(r.arrayBuffer)var a=[`[object Int8Array]`,`[object Uint8Array]`,`[object Uint8ClampedArray]`,`[object Int16Array]`,`[object Uint16Array]`,`[object Int32Array]`,`[object Uint32Array]`,`[object Float32Array]`,`[object Float64Array]`],o=ArrayBuffer.isView||function(e){return e&&a.indexOf(Object.prototype.toString.call(e))>-1};function s(e){if(typeof e!=`string`&&(e=String(e)),/[^a-z0-9\-#$%&'*+.^_`|~!]/i.test(e)||e===``)throw TypeError(`Invalid character in header field name: "`+e+`"`);return e.toLowerCase()}function c(e){return typeof e!=`string`&&(e=String(e)),e}function l(e){var t={next:function(){var t=e.shift();return{done:t===void 0,value:t}}};return r.iterable&&(t[Symbol.iterator]=function(){return t}),t}function u(e){this.map={},e instanceof u?e.forEach(function(e,t){this.append(t,e)},this):Array.isArray(e)?e.forEach(function(e){if(e.length!=2)throw TypeError(`Headers constructor: expected name/value pair to be length 2, found`+e.length);this.append(e[0],e[1])},this):e&&Object.getOwnPropertyNames(e).forEach(function(t){this.append(t,e[t])},this)}u.prototype.append=function(e,t){e=s(e),t=c(t);var n=this.map[e];this.map[e]=n?n+`, `+t:t},u.prototype.delete=function(e){delete this.map[s(e)]},u.prototype.get=function(e){return e=s(e),this.has(e)?this.map[e]:null},u.prototype.has=function(e){return this.map.hasOwnProperty(s(e))},u.prototype.set=function(e,t){this.map[s(e)]=c(t)},u.prototype.forEach=function(e,t){for(var n in this.map)this.map.hasOwnProperty(n)&&e.call(t,this.map[n],n,this)},u.prototype.keys=function(){var e=[];return this.forEach(function(t,n){e.push(n)}),l(e)},u.prototype.values=function(){var e=[];return this.forEach(function(t){e.push(t)}),l(e)},u.prototype.entries=function(){var e=[];return this.forEach(function(t,n){e.push([n,t])}),l(e)},r.iterable&&(u.prototype[Symbol.iterator]=u.prototype.entries);function d(e){if(!e._noBody){if(e.bodyUsed)return Promise.reject(TypeError(`Already read`));e.bodyUsed=!0}}function f(e){return new Promise(function(t,n){e.onload=function(){t(e.result)},e.onerror=function(){n(e.error)}})}function p(e){var t=new FileReader,n=f(t);return t.readAsArrayBuffer(e),n}function m(e){var t=new FileReader,n=f(t),r=/charset=([A-Za-z0-9_-]+)/.exec(e.type),i=r?r[1]:`utf-8`;return t.readAsText(e,i),n}function h(e){for(var t=new Uint8Array(e),n=Array(t.length),r=0;r<t.length;r++)n[r]=String.fromCharCode(t[r]);return n.join(``)}function g(e){if(e.slice)return e.slice(0);var t=new Uint8Array(e.byteLength);return t.set(new Uint8Array(e)),t.buffer}function _(){return this.bodyUsed=!1,this._initBody=function(e){this.bodyUsed=this.bodyUsed,this._bodyInit=e,e?typeof e==`string`?this._bodyText=e:r.blob&&Blob.prototype.isPrototypeOf(e)?this._bodyBlob=e:r.formData&&FormData.prototype.isPrototypeOf(e)?this._bodyFormData=e:r.searchParams&&URLSearchParams.prototype.isPrototypeOf(e)?this._bodyText=e.toString():r.arrayBuffer&&r.blob&&i(e)?(this._bodyArrayBuffer=g(e.buffer),this._bodyInit=new Blob([this._bodyArrayBuffer])):r.arrayBuffer&&(ArrayBuffer.prototype.isPrototypeOf(e)||o(e))?this._bodyArrayBuffer=g(e):this._bodyText=e=Object.prototype.toString.call(e):(this._noBody=!0,this._bodyText=``),this.headers.get(`content-type`)||(typeof e==`string`?this.headers.set(`content-type`,`text/plain;charset=UTF-8`):this._bodyBlob&&this._bodyBlob.type?this.headers.set(`content-type`,this._bodyBlob.type):r.searchParams&&URLSearchParams.prototype.isPrototypeOf(e)&&this.headers.set(`content-type`,`application/x-www-form-urlencoded;charset=UTF-8`))},r.blob&&(this.blob=function(){var e=d(this);if(e)return e;if(this._bodyBlob)return Promise.resolve(this._bodyBlob);if(this._bodyArrayBuffer)return Promise.resolve(new Blob([this._bodyArrayBuffer]));if(this._bodyFormData)throw Error(`could not read FormData body as blob`);return Promise.resolve(new Blob([this._bodyText]))}),this.arrayBuffer=function(){if(this._bodyArrayBuffer)return d(this)||(ArrayBuffer.isView(this._bodyArrayBuffer)?Promise.resolve(this._bodyArrayBuffer.buffer.slice(this._bodyArrayBuffer.byteOffset,this._bodyArrayBuffer.byteOffset+this._bodyArrayBuffer.byteLength)):Promise.resolve(this._bodyArrayBuffer));if(r.blob)return this.blob().then(p);throw Error(`could not read as ArrayBuffer`)},this.text=function(){var e=d(this);if(e)return e;if(this._bodyBlob)return m(this._bodyBlob);if(this._bodyArrayBuffer)return Promise.resolve(h(this._bodyArrayBuffer));if(this._bodyFormData)throw Error(`could not read FormData body as text`);return Promise.resolve(this._bodyText)},r.formData&&(this.formData=function(){return this.text().then(ee)}),this.json=function(){return this.text().then(JSON.parse)},this}var v=[`CONNECT`,`DELETE`,`GET`,`HEAD`,`OPTIONS`,`PATCH`,`POST`,`PUT`,`TRACE`];function y(e){var t=e.toUpperCase();return v.indexOf(t)>-1?t:e}function b(e,t){if(!(this instanceof b))throw TypeError(`Please use the "new" operator, this DOM object constructor cannot be called as a function.`);t||={};var r=t.body;if(e instanceof b){if(e.bodyUsed)throw TypeError(`Already read`);this.url=e.url,this.credentials=e.credentials,t.headers||(this.headers=new u(e.headers)),this.method=e.method,this.mode=e.mode,this.signal=e.signal,!r&&e._bodyInit!=null&&(r=e._bodyInit,e.bodyUsed=!0)}else this.url=String(e);if(this.credentials=t.credentials||this.credentials||`same-origin`,(t.headers||!this.headers)&&(this.headers=new u(t.headers)),this.method=y(t.method||this.method||`GET`),this.mode=t.mode||this.mode||null,this.signal=t.signal||this.signal||function(){if(`AbortController`in n)return new AbortController().signal}(),this.referrer=null,(this.method===`GET`||this.method===`HEAD`)&&r)throw TypeError(`Body not allowed for GET or HEAD requests`);if(this._initBody(r),(this.method===`GET`||this.method===`HEAD`)&&(t.cache===`no-store`||t.cache===`no-cache`)){var i=/([?&])_=[^&]*/;i.test(this.url)?this.url=this.url.replace(i,`$1_=`+new Date().getTime()):this.url+=(/\?/.test(this.url)?`&`:`?`)+`_=`+new Date().getTime()}}b.prototype.clone=function(){return new b(this,{body:this._bodyInit})};function ee(e){var t=new FormData;return e.trim().split(`&`).forEach(function(e){if(e){var n=e.split(`=`),r=n.shift().replace(/\+/g,` `),i=n.join(`=`).replace(/\+/g,` `);t.append(decodeURIComponent(r),decodeURIComponent(i))}}),t}function te(e){var t=new u;return e.replace(/\r?\n[\t ]+/g,` `).split(`\r`).map(function(e){return e.indexOf(`
`)===0?e.substr(1,e.length):e}).forEach(function(e){var n=e.split(`:`),r=n.shift().trim();if(r){var i=n.join(`:`).trim();try{t.append(r,i)}catch(e){console.warn(`Response `+e.message)}}}),t}_.call(b.prototype);function x(e,t){if(!(this instanceof x))throw TypeError(`Please use the "new" operator, this DOM object constructor cannot be called as a function.`);if(t||={},this.type=`default`,this.status=t.status===void 0?200:t.status,this.status<200||this.status>599)throw RangeError(`Failed to construct 'Response': The status provided (0) is outside the range [200, 599].`);this.ok=this.status>=200&&this.status<300,this.statusText=t.statusText===void 0?``:``+t.statusText,this.headers=new u(t.headers),this.url=t.url||``,this._initBody(e)}_.call(x.prototype),x.prototype.clone=function(){return new x(this._bodyInit,{status:this.status,statusText:this.statusText,headers:new u(this.headers),url:this.url})},x.error=function(){var e=new x(null,{status:200,statusText:``});return e.ok=!1,e.status=0,e.type=`error`,e};var ne=[301,302,303,307,308];x.redirect=function(e,t){if(ne.indexOf(t)===-1)throw RangeError(`Invalid status code`);return new x(null,{status:t,headers:{location:e}})},t.DOMException=n.DOMException;try{new t.DOMException}catch{t.DOMException=function(e,t){this.message=e,this.name=t,this.stack=Error(e).stack},t.DOMException.prototype=Object.create(Error.prototype),t.DOMException.prototype.constructor=t.DOMException}function re(e,i){return new Promise(function(a,o){var l=new b(e,i);if(l.signal&&l.signal.aborted)return o(new t.DOMException(`Aborted`,`AbortError`));var d=new XMLHttpRequest;function f(){d.abort()}d.onload=function(){var e={statusText:d.statusText,headers:te(d.getAllResponseHeaders()||``)};l.url.indexOf(`file://`)===0&&(d.status<200||d.status>599)?e.status=200:e.status=d.status,e.url=`responseURL`in d?d.responseURL:e.headers.get(`X-Request-URL`);var t=`response`in d?d.response:d.responseText;setTimeout(function(){a(new x(t,e))},0)},d.onerror=function(){setTimeout(function(){o(TypeError(`Network request failed`))},0)},d.ontimeout=function(){setTimeout(function(){o(TypeError(`Network request timed out`))},0)},d.onabort=function(){setTimeout(function(){o(new t.DOMException(`Aborted`,`AbortError`))},0)};function p(e){try{return e===``&&n.location.href?n.location.href:e}catch{return e}}if(d.open(l.method,p(l.url),!0),l.credentials===`include`?d.withCredentials=!0:l.credentials===`omit`&&(d.withCredentials=!1),`responseType`in d&&(r.blob?d.responseType=`blob`:r.arrayBuffer&&(d.responseType=`arraybuffer`)),i&&typeof i.headers==`object`&&!(i.headers instanceof u||n.Headers&&i.headers instanceof n.Headers)){var m=[];Object.getOwnPropertyNames(i.headers).forEach(function(e){m.push(s(e)),d.setRequestHeader(e,c(i.headers[e]))}),l.headers.forEach(function(e,t){m.indexOf(t)===-1&&d.setRequestHeader(t,e)})}else l.headers.forEach(function(e,t){d.setRequestHeader(t,e)});l.signal&&(l.signal.addEventListener(`abort`,f),d.onreadystatechange=function(){d.readyState===4&&l.signal.removeEventListener(`abort`,f)}),d.send(l._bodyInit===void 0?null:l._bodyInit)})}return re.polyfill=!0,n.fetch||(n.fetch=re,n.Headers=u,n.Request=b,n.Response=x),t.Headers=u,t.Request=b,t.Response=x,t.fetch=re,Object.defineProperty(t,`__esModule`,{value:!0}),t})({})})(r),r.fetch.ponyfill=!0,delete r.fetch.polyfill;var i=n.fetch?n:r;e=i.fetch,e.default=i.fetch,e.fetch=i.fetch,e.Headers=i.Headers,e.Request=i.Request,e.Response=i.Response,t.exports=e}))()),Xt=Object.defineProperty,Zt=Object.defineProperties,Qt=Object.getOwnPropertyDescriptors,$t=Object.getOwnPropertySymbols,en=Object.prototype.hasOwnProperty,tn=Object.prototype.propertyIsEnumerable,nn=(e,t,n)=>t in e?Xt(e,t,{enumerable:!0,configurable:!0,writable:!0,value:n}):e[t]=n,rn=(e,t)=>{for(var n in t||={})en.call(t,n)&&nn(e,n,t[n]);if($t)for(var n of $t(t))tn.call(t,n)&&nn(e,n,t[n]);return e},an=(e,t)=>Zt(e,Qt(t)),on={headers:{Accept:`application/json`,"Content-Type":`application/json`},method:`POST`},sn=10,cn=class{constructor(e,t=!1){if(this.url=e,this.disableProviderPing=t,this.events=new qt.EventEmitter,this.isAvailable=!1,this.registering=!1,!Lt(e))throw Error(`Provided URL is not compatible with HTTP connection: ${e}`);this.url=e,this.disableProviderPing=t}get connected(){return this.isAvailable}get connecting(){return this.registering}on(e,t){this.events.on(e,t)}once(e,t){this.events.once(e,t)}off(e,t){this.events.off(e,t)}removeListener(e,t){this.events.removeListener(e,t)}async open(e=this.url){await this.register(e)}async close(){if(!this.isAvailable)throw Error(`Connection already closed`);this.onClose()}async send(e){this.isAvailable||await this.register();try{let t=g(e),n=await(await(0,Yt.default)(this.url,an(rn({},on),{body:t}))).json();this.onPayload({data:n})}catch(t){this.onError(e.id,t)}}async register(e=this.url){if(!Lt(e))throw Error(`Provided URL is not compatible with HTTP connection: ${e}`);if(this.registering){let e=this.events.getMaxListeners();return(this.events.listenerCount(`register_error`)>=e||this.events.listenerCount(`open`)>=e)&&this.events.setMaxListeners(e+1),new Promise((e,t)=>{this.events.once(`register_error`,e=>{this.resetMaxListeners(),t(e)}),this.events.once(`open`,()=>{if(this.resetMaxListeners(),typeof this.isAvailable>`u`)return t(Error(`HTTP connection is missing or invalid`));e()})})}this.url=e,this.registering=!0;try{if(!this.disableProviderPing){let t=g({id:1,jsonrpc:`2.0`,method:`test`,params:[]});await(0,Yt.default)(e,an(rn({},on),{body:t}))}this.onOpen()}catch(e){let t=this.parseError(e);throw this.events.emit(`register_error`,t),this.onClose(),t}}onOpen(){this.isAvailable=!0,this.registering=!1,this.events.emit(`open`)}onClose(){this.isAvailable=!1,this.registering=!1,this.events.emit(`close`)}onPayload(e){if(typeof e.data>`u`)return;let t=typeof e.data==`string`?h(e.data):e.data;this.events.emit(`payload`,t)}onError(e,t){let n=this.parseError(t),r=St(e,n.message||n.toString());this.events.emit(`payload`,r)}parseError(e,t=this.url){return ze(e,t,`HTTP`)}resetMaxListeners(){this.events.getMaxListeners()>sn&&this.events.setMaxListeners(sn)}},ln={caipNetworkIdToNumber(e){return e?Number(e.split(`:`)[1]):void 0},parseEvmChainId(e){return typeof e==`string`?this.caipNetworkIdToNumber(e):e},getNetworksByNamespace(e,t){return e?.filter(e=>e.chainNamespace===t)||[]},getFirstNetworkByNamespace(e,t){return this.getNetworksByNamespace(e,t)[0]}},un=20,dn=1,fn=1e6,pn=1e6,mn=-7,hn=21,gn=!1,_n=`[big.js] `,vn=_n+`Invalid `,yn=vn+`decimal places`,bn=vn+`rounding mode`,xn=_n+`Division by zero`,S={},Sn=void 0,Cn=/^-?(\d+(\.\d*)?|\.\d+)(e[+-]?\d+)?$/i;function wn(){function e(t){var n=this;if(!(n instanceof e))return t===Sn?wn():new e(t);if(t instanceof e)n.s=t.s,n.e=t.e,n.c=t.c.slice();else{if(typeof t!=`string`){if(e.strict===!0&&typeof t!=`bigint`)throw TypeError(vn+`value`);t=t===0&&1/t<0?`-0`:String(t)}Tn(n,t)}n.constructor=e}return e.prototype=S,e.DP=un,e.RM=dn,e.NE=mn,e.PE=hn,e.strict=gn,e.roundDown=0,e.roundHalfUp=1,e.roundHalfEven=2,e.roundUp=3,e}function Tn(e,t){var n,r,i;if(!Cn.test(t))throw Error(vn+`number`);for(e.s=t.charAt(0)==`-`?(t=t.slice(1),-1):1,(n=t.indexOf(`.`))>-1&&(t=t.replace(`.`,``)),(r=t.search(/e/i))>0?(n<0&&(n=r),n+=+t.slice(r+1),t=t.substring(0,r)):n<0&&(n=t.length),i=t.length,r=0;r<i&&t.charAt(r)==`0`;)++r;if(r==i)e.c=[e.e=0];else{for(;i>0&&t.charAt(--i)==`0`;);for(e.e=n-r-1,e.c=[],n=0;r<=i;)e.c[n++]=+t.charAt(r++)}return e}function En(e,t,n,r){var i=e.c;if(n===Sn&&(n=e.constructor.RM),n!==0&&n!==1&&n!==2&&n!==3)throw Error(bn);if(t<1)r=n===3&&(r||!!i[0])||t===0&&(n===1&&i[0]>=5||n===2&&(i[0]>5||i[0]===5&&(r||i[1]!==Sn))),i.length=1,r?(e.e=e.e-t+1,i[0]=1):i[0]=e.e=0;else if(t<i.length){if(r=n===1&&i[t]>=5||n===2&&(i[t]>5||i[t]===5&&(r||i[t+1]!==Sn||i[t-1]&1))||n===3&&(r||!!i[0]),i.length=t,r){for(;++i[--t]>9;)if(i[t]=0,t===0){++e.e,i.unshift(1);break}}for(t=i.length;!i[--t];)i.pop()}return e}function Dn(e,t,n){var r=e.e,i=e.c.join(``),a=i.length;if(t)i=i.charAt(0)+(a>1?`.`+i.slice(1):``)+(r<0?`e`:`e+`)+r;else if(r<0){for(;++r;)i=`0`+i;i=`0.`+i}else if(r>0)if(++r>a)for(r-=a;r--;)i+=`0`;else r<a&&(i=i.slice(0,r)+`.`+i.slice(r));else a>1&&(i=i.charAt(0)+`.`+i.slice(1));return e.s<0&&n?`-`+i:i}S.abs=function(){var e=new this.constructor(this);return e.s=1,e},S.cmp=function(e){var t,n=this,r=n.c,i=(e=new n.constructor(e)).c,a=n.s,o=e.s,s=n.e,c=e.e;if(!r[0]||!i[0])return r[0]?a:i[0]?-o:0;if(a!=o)return a;if(t=a<0,s!=c)return s>c^t?1:-1;for(o=(s=r.length)<(c=i.length)?s:c,a=-1;++a<o;)if(r[a]!=i[a])return r[a]>i[a]^t?1:-1;return s==c?0:s>c^t?1:-1},S.div=function(e){var t=this,n=t.constructor,r=t.c,i=(e=new n(e)).c,a=t.s==e.s?1:-1,o=n.DP;if(o!==~~o||o<0||o>fn)throw Error(yn);if(!i[0])throw Error(xn);if(!r[0])return e.s=a,e.c=[e.e=0],e;var s,c,l,u,d,f=i.slice(),p=s=i.length,m=r.length,h=r.slice(0,s),g=h.length,_=e,v=_.c=[],y=0,b=o+(_.e=t.e-e.e)+1;for(_.s=a,a=b<0?0:b,f.unshift(0);g++<s;)h.push(0);do{for(l=0;l<10;l++){if(s!=(g=h.length))u=s>g?1:-1;else for(d=-1,u=0;++d<s;)if(i[d]!=h[d]){u=i[d]>h[d]?1:-1;break}if(u<0){for(c=g==s?i:f;g;){if(h[--g]<c[g]){for(d=g;d&&!h[--d];)h[d]=9;--h[d],h[g]+=10}h[g]-=c[g]}for(;!h[0];)h.shift()}else break}v[y++]=u?l:++l,h[0]&&u?h[g]=r[p]||0:h=[r[p]]}while((p++<m||h[0]!==Sn)&&a--);return!v[0]&&y!=1&&(v.shift(),_.e--,b--),y>b&&En(_,b,n.RM,h[0]!==Sn),_},S.eq=function(e){return this.cmp(e)===0},S.gt=function(e){return this.cmp(e)>0},S.gte=function(e){return this.cmp(e)>-1},S.lt=function(e){return this.cmp(e)<0},S.lte=function(e){return this.cmp(e)<1},S.minus=S.sub=function(e){var t,n,r,i,a=this,o=a.constructor,s=a.s,c=(e=new o(e)).s;if(s!=c)return e.s=-c,a.plus(e);var l=a.c.slice(),u=a.e,d=e.c,f=e.e;if(!l[0]||!d[0])return d[0]?e.s=-c:l[0]?e=new o(a):e.s=1,e;if(s=u-f){for((i=s<0)?(s=-s,r=l):(f=u,r=d),r.reverse(),c=s;c--;)r.push(0);r.reverse()}else for(n=((i=l.length<d.length)?l:d).length,s=c=0;c<n;c++)if(l[c]!=d[c]){i=l[c]<d[c];break}if(i&&(r=l,l=d,d=r,e.s=-e.s),(c=(n=d.length)-(t=l.length))>0)for(;c--;)l[t++]=0;for(c=t;n>s;){if(l[--n]<d[n]){for(t=n;t&&!l[--t];)l[t]=9;--l[t],l[n]+=10}l[n]-=d[n]}for(;l[--c]===0;)l.pop();for(;l[0]===0;)l.shift(),--f;return l[0]||(e.s=1,l=[f=0]),e.c=l,e.e=f,e},S.mod=function(e){var t,n=this,r=n.constructor,i=n.s,a=(e=new r(e)).s;if(!e.c[0])throw Error(xn);return n.s=e.s=1,t=e.cmp(n)==1,n.s=i,e.s=a,t?new r(n):(i=r.DP,a=r.RM,r.DP=r.RM=0,n=n.div(e),r.DP=i,r.RM=a,this.minus(n.times(e)))},S.neg=function(){var e=new this.constructor(this);return e.s=-e.s,e},S.plus=S.add=function(e){var t,n,r,i=this,a=i.constructor;if(e=new a(e),i.s!=e.s)return e.s=-e.s,i.minus(e);var o=i.e,s=i.c,c=e.e,l=e.c;if(!s[0]||!l[0])return l[0]||(s[0]?e=new a(i):e.s=i.s),e;if(s=s.slice(),t=o-c){for(t>0?(c=o,r=l):(t=-t,r=s),r.reverse();t--;)r.push(0);r.reverse()}for(s.length-l.length<0&&(r=l,l=s,s=r),t=l.length,n=0;t;s[t]%=10)n=(s[--t]=s[t]+l[t]+n)/10|0;for(n&&(s.unshift(n),++c),t=s.length;s[--t]===0;)s.pop();return e.c=s,e.e=c,e},S.pow=function(e){var t=this,n=new t.constructor(`1`),r=n,i=e<0;if(e!==~~e||e<-pn||e>pn)throw Error(vn+`exponent`);for(i&&(e=-e);e&1&&(r=r.times(t)),e>>=1,e;)t=t.times(t);return i?n.div(r):r},S.prec=function(e,t){if(e!==~~e||e<1||e>fn)throw Error(vn+`precision`);return En(new this.constructor(this),e,t)},S.round=function(e,t){if(e===Sn)e=0;else if(e!==~~e||e<-fn||e>fn)throw Error(yn);return En(new this.constructor(this),e+this.e+1,t)},S.sqrt=function(){var e,t,n,r=this,i=r.constructor,a=r.s,o=r.e,s=new i(`0.5`);if(!r.c[0])return new i(r);if(a<0)throw Error(_n+`No square root`);a=Math.sqrt(+Dn(r,!0,!0)),a===0||a===1/0?(t=r.c.join(``),t.length+o&1||(t+=`0`),a=Math.sqrt(t),o=((o+1)/2|0)-(o<0||o&1),e=new i((a==1/0?`5e`:(a=a.toExponential()).slice(0,a.indexOf(`e`)+1))+o)):e=new i(a+``),o=e.e+(i.DP+=4);do n=e,e=s.times(n.plus(r.div(n)));while(n.c.slice(0,o).join(``)!==e.c.slice(0,o).join(``));return En(e,(i.DP-=4)+e.e+1,i.RM)},S.times=S.mul=function(e){var t,n=this,r=n.constructor,i=n.c,a=(e=new r(e)).c,o=i.length,s=a.length,c=n.e,l=e.e;if(e.s=n.s==e.s?1:-1,!i[0]||!a[0])return e.c=[e.e=0],e;for(e.e=c+l,o<s&&(t=i,i=a,a=t,l=o,o=s,s=l),t=Array(l=o+s);l--;)t[l]=0;for(c=s;c--;){for(s=0,l=o+c;l>c;)s=t[l]+a[c]*i[l-c-1]+s,t[l--]=s%10,s=s/10|0;t[l]=s}for(s?++e.e:t.shift(),c=t.length;!t[--c];)t.pop();return e.c=t,e},S.toExponential=function(e,t){var n=this,r=n.c[0];if(e!==Sn){if(e!==~~e||e<0||e>fn)throw Error(yn);for(n=En(new n.constructor(n),++e,t);n.c.length<e;)n.c.push(0)}return Dn(n,!0,!!r)},S.toFixed=function(e,t){var n=this,r=n.c[0];if(e!==Sn){if(e!==~~e||e<0||e>fn)throw Error(yn);for(n=En(new n.constructor(n),e+n.e+1,t),e=e+n.e+1;n.c.length<e;)n.c.push(0)}return Dn(n,!1,!!r)},S[Symbol.for(`nodejs.util.inspect.custom`)]=S.toJSON=S.toString=function(){var e=this,t=e.constructor;return Dn(e,e.e<=t.NE||e.e>=t.PE,!!e.c[0])},S.toNumber=function(){var e=+Dn(this,!0,!0);if(this.constructor.strict===!0&&!this.eq(e.toString()))throw Error(_n+`Imprecise conversion`);return e},S.toPrecision=function(e,t){var n=this,r=n.constructor,i=n.c[0];if(e!==Sn){if(e!==~~e||e<1||e>fn)throw Error(vn+`precision`);for(n=En(new r(n),e,t);n.c.length<e;)n.c.push(0)}return Dn(n,e<=n.e||n.e<=r.NE||n.e>=r.PE,!!i)},S.valueOf=function(){var e=this,t=e.constructor;if(t.strict===!0)throw Error(_n+`valueOf disallowed`);return Dn(e,e.e<=t.NE||e.e>=t.PE,!0)};var On=wn(),kn={bigNumber(e){return e?new On(e):new On(0)},multiply(e,t){if(e===void 0||t===void 0)return new On(0);let n=new On(e),r=new On(t);return n.times(r)},formatNumberToLocalString(e,t=2){return e===void 0?`0.00`:typeof e==`number`?e.toLocaleString(`en-US`,{maximumFractionDigits:t,minimumFractionDigits:t}):parseFloat(e).toLocaleString(`en-US`,{maximumFractionDigits:t,minimumFractionDigits:t})},parseLocalStringToNumber(e){return e===void 0?0:parseFloat(e.replace(/,/gu,``))}},An=[{type:`function`,name:`transfer`,stateMutability:`nonpayable`,inputs:[{name:`_to`,type:`address`},{name:`_value`,type:`uint256`}],outputs:[{name:``,type:`bool`}]},{type:`function`,name:`transferFrom`,stateMutability:`nonpayable`,inputs:[{name:`_from`,type:`address`},{name:`_to`,type:`address`},{name:`_value`,type:`uint256`}],outputs:[{name:``,type:`bool`}]}],jn=[{type:`function`,name:`approve`,stateMutability:`nonpayable`,inputs:[{name:`spender`,type:`address`},{name:`amount`,type:`uint256`}],outputs:[{type:`bool`}]}],Mn=[{type:`function`,name:`transfer`,stateMutability:`nonpayable`,inputs:[{name:`recipient`,type:`address`},{name:`amount`,type:`uint256`}],outputs:[]},{type:`function`,name:`transferFrom`,stateMutability:`nonpayable`,inputs:[{name:`sender`,type:`address`},{name:`recipient`,type:`address`},{name:`amount`,type:`uint256`}],outputs:[{name:``,type:`bool`}]}],C={WC_NAME_SUFFIX:`.reown.id`,WC_NAME_SUFFIX_LEGACY:`.wcn.id`,BLOCKCHAIN_API_RPC_URL:`https://rpc.walletconnect.org`,PULSE_API_URL:`https://pulse.walletconnect.org`,W3M_API_URL:`https://api.web3modal.org`,CONNECTOR_ID:{WALLET_CONNECT:`walletConnect`,INJECTED:`injected`,WALLET_STANDARD:`announced`,COINBASE:`coinbaseWallet`,COINBASE_SDK:`coinbaseWalletSDK`,SAFE:`safe`,LEDGER:`ledger`,OKX:`okx`,EIP6963:`eip6963`,AUTH:`ID_AUTH`},CONNECTOR_NAMES:{AUTH:`Auth`},AUTH_CONNECTOR_SUPPORTED_CHAINS:[`eip155`,`solana`],LIMITS:{PENDING_TRANSACTIONS:99},CHAIN:{EVM:`eip155`,SOLANA:`solana`,POLKADOT:`polkadot`,BITCOIN:`bip122`},CHAIN_NAME_MAP:{eip155:`EVM Networks`,solana:`Solana`,polkadot:`Polkadot`,bip122:`Bitcoin`},ADAPTER_TYPES:{BITCOIN:`bitcoin`,SOLANA:`solana`,WAGMI:`wagmi`,ETHERS:`ethers`,ETHERS5:`ethers5`},USDT_CONTRACT_ADDRESSES:[`0xdac17f958d2ee523a2206206994597c13d831ec7`,`0xc2132d05d31c914a87c6611c10748aeb04b58e8f`,`0x9702230a8ea53601f5cd2dc00fdbc13d4df4a8c7`,`0x919C1c267BC06a7039e03fcc2eF738525769109c`,`0x48065fbBE25f71C9282ddf5e1cD6D6A887483D5e`,`0x55d398326f99059fF775485246999027B3197955`,`0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9`],HTTP_STATUS_CODES:{SERVICE_UNAVAILABLE:503,FORBIDDEN:403},UNSUPPORTED_NETWORK_NAME:`Unknown Network`},Nn={getERC20Abi:e=>C.USDT_CONTRACT_ADDRESSES.includes(e)?Mn:An,getSwapAbi:()=>jn},w={WALLET_ID:`@appkit/wallet_id`,WALLET_NAME:`@appkit/wallet_name`,SOLANA_WALLET:`@appkit/solana_wallet`,SOLANA_CAIP_CHAIN:`@appkit/solana_caip_chain`,ACTIVE_CAIP_NETWORK_ID:`@appkit/active_caip_network_id`,CONNECTED_SOCIAL:`@appkit/connected_social`,CONNECTED_SOCIAL_USERNAME:`@appkit-wallet/SOCIAL_USERNAME`,RECENT_WALLETS:`@appkit/recent_wallets`,DEEPLINK_CHOICE:`WALLETCONNECT_DEEPLINK_CHOICE`,ACTIVE_NAMESPACE:`@appkit/active_namespace`,CONNECTED_NAMESPACES:`@appkit/connected_namespaces`,CONNECTION_STATUS:`@appkit/connection_status`,SIWX_AUTH_TOKEN:`@appkit/siwx-auth-token`,SIWX_NONCE_TOKEN:`@appkit/siwx-nonce-token`,TELEGRAM_SOCIAL_PROVIDER:`@appkit/social_provider`,NATIVE_BALANCE_CACHE:`@appkit/native_balance_cache`,PORTFOLIO_CACHE:`@appkit/portfolio_cache`,ENS_CACHE:`@appkit/ens_cache`,IDENTITY_CACHE:`@appkit/identity_cache`};function Pn(e){if(!e)throw Error(`Namespace is required for CONNECTED_CONNECTOR_ID`);return`@appkit/${e}:connected_connector_id`}var T={setItem(e,t){Fn()&&t!==void 0&&localStorage.setItem(e,t)},getItem(e){if(Fn())return localStorage.getItem(e)||void 0},removeItem(e){Fn()&&localStorage.removeItem(e)},clear(){Fn()&&localStorage.clear()}};function Fn(){return typeof window<`u`&&typeof localStorage<`u`}function In(e,t){return t===`light`?{"--w3m-accent":e?.[`--w3m-accent`]||`hsla(231, 100%, 70%, 1)`,"--w3m-background":`#fff`}:{"--w3m-accent":e?.[`--w3m-accent`]||`hsla(230, 100%, 67%, 1)`,"--w3m-background":`#121313`}}var Ln=Symbol(),Rn=Symbol(),zn=`a`,Bn=`w`,Vn=(e,t)=>new Proxy(e,t),Hn=Object.getPrototypeOf,Un=new WeakMap,Wn=e=>e&&(Un.has(e)?Un.get(e):Hn(e)===Object.prototype||Hn(e)===Array.prototype),Gn=e=>typeof e==`object`&&!!e,Kn=e=>{if(Array.isArray(e))return Array.from(e);let t=Object.getOwnPropertyDescriptors(e);return Object.values(t).forEach(e=>{e.configurable=!0}),Object.create(Hn(e),t)},qn=e=>e[Rn]||e,Jn=(e,t,n,r)=>{if(!Wn(e))return e;let i=r&&r.get(e);if(!i){let t=qn(e);i=(e=>Object.values(Object.getOwnPropertyDescriptors(e)).some(e=>!e.configurable&&!e.writable))(t)?[t,Kn(t)]:[t],r?.set(e,i)}let[a,o]=i,s=n&&n.get(a);return s&&s[1].f===!!o||(s=((e,t)=>{let n={f:t},r=!1,i=(t,i)=>{if(!r){let r=n[zn].get(e);if(r||(r={},n[zn].set(e,r)),t===Bn)r[Bn]=!0;else{let e=r[t];e||(e=new Set,r[t]=e),e.add(i)}}},a={get:(t,r)=>r===Rn?e:(i(`k`,r),Jn(Reflect.get(t,r),n[zn],n.c,n.t)),has:(t,a)=>a===Ln?(r=!0,n[zn].delete(e),!0):(i(`h`,a),Reflect.has(t,a)),getOwnPropertyDescriptor:(e,t)=>(i(`o`,t),Reflect.getOwnPropertyDescriptor(e,t)),ownKeys:e=>(i(Bn),Reflect.ownKeys(e))};return t&&(a.set=a.deleteProperty=()=>!1),[a,n]})(a,!!o),s[1].p=Vn(o||a,s[0]),n&&n.set(a,s)),s[1][zn]=t,s[1].c=n,s[1].t=r,s[1].p},Yn=(e,t,n,r,i=Object.is)=>{if(i(e,t))return!1;if(!Gn(e)||!Gn(t))return!0;let a=n.get(qn(e));if(!a)return!0;if(r){let n=r.get(e);if(n&&n.n===t)return n.g;r.set(e,{n:t,g:!1})}let o=null;try{for(let n of a.h||[])if(o=Reflect.has(e,n)!==Reflect.has(t,n),o)return o;if(!0===a[Bn]){if(o=((e,t)=>{let n=Reflect.ownKeys(e),r=Reflect.ownKeys(t);return n.length!==r.length||n.some((e,t)=>e!==r[t])})(e,t),o)return o}else for(let n of a.o||[])if(o=!!Reflect.getOwnPropertyDescriptor(e,n)!=!!Reflect.getOwnPropertyDescriptor(t,n),o)return o;for(let s of a.k||[])if(o=Yn(e[s],t[s],n,r,i),o)return o;return o===null&&(o=!0),o}finally{r&&r.set(e,{n:t,g:o})}},Xn=e=>Wn(e)&&e[Rn]||null,Zn=(e,t=!0)=>{Un.set(e,t)},Qn=e=>typeof e==`object`&&!!e,$n=new WeakMap,er=new WeakSet,[tr]=((e=Object.is,t=(e,t)=>new Proxy(e,t),n=e=>Qn(e)&&!er.has(e)&&(Array.isArray(e)||!(Symbol.iterator in e))&&!(e instanceof WeakMap)&&!(e instanceof WeakSet)&&!(e instanceof Error)&&!(e instanceof Number)&&!(e instanceof Date)&&!(e instanceof String)&&!(e instanceof RegExp)&&!(e instanceof ArrayBuffer),r=e=>{switch(e.status){case`fulfilled`:return e.value;case`rejected`:throw e.reason;default:throw e}},i=new WeakMap,a=(e,t,n=r)=>{let o=i.get(e);if(o?.[0]===t)return o[1];let s=Array.isArray(e)?[]:Object.create(Object.getPrototypeOf(e));return Zn(s,!0),i.set(e,[t,s]),Reflect.ownKeys(e).forEach(t=>{if(Object.getOwnPropertyDescriptor(s,t))return;let r=Reflect.get(e,t),{enumerable:i}=Reflect.getOwnPropertyDescriptor(e,t),o={value:r,enumerable:i,configurable:!0};if(er.has(r))Zn(r,!1);else if(r instanceof Promise)delete o.value,o.get=()=>n(r);else if($n.has(r)){let[e,t]=$n.get(r);o.value=a(e,t(),n)}Object.defineProperty(s,t,o)}),Object.preventExtensions(s)},o=new WeakMap,s=[1,1],c=r=>{if(!Qn(r))throw Error(`object required`);let i=o.get(r);if(i)return i;let l=s[0],u=new Set,d=(e,t=++s[0])=>{l!==t&&(l=t,u.forEach(n=>n(e,t)))},f=s[1],p=(e=++s[1])=>(f!==e&&!u.size&&(f=e,h.forEach(([t])=>{let n=t[1](e);n>l&&(l=n)})),l),m=e=>(t,n)=>{let r=[...t];r[1]=[e,...r[1]],d(r,n)},h=new Map,g=(e,t)=>{if(u.size){let n=t[3](m(e));h.set(e,[t,n])}else h.set(e,[t])},_=e=>{var t;let n=h.get(e);n&&(h.delete(e),(t=n[1])==null||t.call(n))},v=e=>(u.add(e),u.size===1&&h.forEach(([e,t],n)=>{let r=e[3](m(n));h.set(n,[e,r])}),()=>{u.delete(e),u.size===0&&h.forEach(([e,t],n)=>{t&&(t(),h.set(n,[e]))})}),y=Array.isArray(r)?[]:Object.create(Object.getPrototypeOf(r)),b=t(y,{deleteProperty(e,t){let n=Reflect.get(e,t);_(t);let r=Reflect.deleteProperty(e,t);return r&&d([`delete`,[t],n]),r},set(t,r,i,a){let s=Reflect.has(t,r),l=Reflect.get(t,r,a);if(s&&(e(l,i)||o.has(i)&&e(l,o.get(i))))return!0;_(r),Qn(i)&&(i=Xn(i)||i);let u=i;if(i instanceof Promise)i.then(e=>{i.status=`fulfilled`,i.value=e,d([`resolve`,[r],e])}).catch(e=>{i.status=`rejected`,i.reason=e,d([`reject`,[r],e])});else{!$n.has(i)&&n(i)&&(u=c(i));let e=!er.has(u)&&$n.get(u);e&&g(r,e)}return Reflect.set(t,r,u,a),d([`set`,[r],i,l]),!0}});o.set(r,b);let ee=[y,p,a,v];return $n.set(b,ee),Reflect.ownKeys(r).forEach(e=>{let t=Object.getOwnPropertyDescriptor(r,e);`value`in t&&(b[e]=r[e],delete t.value,delete t.writable),Object.defineProperty(y,e,t)}),b})=>[c,$n,er,e,t,n,r,i,a,o,s])();function E(e={}){return tr(e)}function D(e,t,n){let r=$n.get(e),i,a=[],o=r[3],s=!1,c=o(e=>{if(a.push(e),n){t(a.splice(0));return}i||=Promise.resolve().then(()=>{i=void 0,s&&t(a.splice(0))})});return s=!0,()=>{s=!1,c()}}function nr(e,t){let[n,r,i]=$n.get(e);return i(n,r(),t)}function rr(e){return er.add(e),e}function ir(e,t,n,r){let i=e[t];return D(e,()=>{let r=e[t];Object.is(i,r)||n(i=r)},r)}function ar(e){let t=E({data:Array.from(e||[]),has(e){return this.data.some(t=>t[0]===e)},set(e,t){let n=this.data.find(t=>t[0]===e);return n?n[1]=t:this.data.push([e,t]),this},get(e){return this.data.find(t=>t[0]===e)?.[1]},delete(e){let t=this.data.findIndex(t=>t[0]===e);return t===-1?!1:(this.data.splice(t,1),!0)},clear(){this.data.splice(0)},get size(){return this.data.length},toJSON(){return new Map(this.data)},forEach(e){this.data.forEach(t=>{e(t[1],t[0],this)})},keys(){return this.data.map(e=>e[0]).values()},values(){return this.data.map(e=>e[1]).values()},entries(){return new Map(this.data).entries()},get[Symbol.toStringTag](){return`Map`},[Symbol.iterator](){return this.entries()}});return Object.defineProperties(t,{data:{enumerable:!1},size:{enumerable:!1},toJSON:{enumerable:!1}}),Object.seal(t),t}var or=(typeof process<`u`?{}.NEXT_PUBLIC_SECURE_SITE_ORIGIN:void 0)||`https://secure.walletconnect.org`,O={FOUR_MINUTES_MS:24e4,TEN_SEC_MS:1e4,FIVE_SEC_MS:5e3,THREE_SEC_MS:3e3,ONE_SEC_MS:1e3,SECURE_SITE:or,SECURE_SITE_DASHBOARD:`${or}/dashboard`,SECURE_SITE_FAVICON:`${or}/images/favicon.png`,RESTRICTED_TIMEZONES:[`ASIA/SHANGHAI`,`ASIA/URUMQI`,`ASIA/CHONGQING`,`ASIA/HARBIN`,`ASIA/KASHGAR`,`ASIA/MACAU`,`ASIA/HONG_KONG`,`ASIA/MACAO`,`ASIA/BEIJING`,`ASIA/HARBIN`],WC_COINBASE_PAY_SDK_CHAINS:[`ethereum`,`arbitrum`,`polygon`,`berachain`,`avalanche-c-chain`,`optimism`,`celo`,`base`],WC_COINBASE_PAY_SDK_FALLBACK_CHAIN:`ethereum`,WC_COINBASE_PAY_SDK_CHAIN_NAME_MAP:{Ethereum:`ethereum`,"Arbitrum One":`arbitrum`,Polygon:`polygon`,Berachain:`berachain`,Avalanche:`avalanche-c-chain`,"OP Mainnet":`optimism`,Celo:`celo`,Base:`base`},WC_COINBASE_ONRAMP_APP_ID:`bf18c88d-495a-463b-b249-0b9d3656cf5e`,SWAP_SUGGESTED_TOKENS:`ETH.UNI.1INCH.AAVE.SOL.ADA.AVAX.DOT.LINK.NITRO.GAIA.MILK.TRX.NEAR.GNO.WBTC.DAI.WETH.USDC.USDT.ARB.BAL.BICO.CRV.ENS.MATIC.OP`.split(`.`),SWAP_POPULAR_TOKENS:`ETH.UNI.1INCH.AAVE.SOL.ADA.AVAX.DOT.LINK.NITRO.GAIA.MILK.TRX.NEAR.GNO.WBTC.DAI.WETH.USDC.USDT.ARB.BAL.BICO.CRV.ENS.MATIC.OP.METAL.DAI.CHAMP.WOLF.SALE.BAL.BUSD.MUST.BTCpx.ROUTE.HEX.WELT.amDAI.VSQ.VISION.AURUM.pSP.SNX.VC.LINK.CHP.amUSDT.SPHERE.FOX.GIDDY.GFC.OMEN.OX_OLD.DE.WNT`.split(`.`),BALANCE_SUPPORTED_CHAINS:[`eip155`,`solana`],SWAP_SUPPORTED_NETWORKS:[`eip155:1`,`eip155:42161`,`eip155:10`,`eip155:324`,`eip155:8453`,`eip155:56`,`eip155:137`,`eip155:100`,`eip155:43114`,`eip155:250`,`eip155:8217`,`eip155:1313161554`],NAMES_SUPPORTED_CHAIN_NAMESPACES:[`eip155`],ONRAMP_SUPPORTED_CHAIN_NAMESPACES:[`eip155`,`solana`],ACTIVITY_ENABLED_CHAIN_NAMESPACES:[`eip155`,`solana`],NATIVE_TOKEN_ADDRESS:{eip155:`0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee`,solana:`So11111111111111111111111111111111111111111`,polkadot:`0x`,bip122:`0x`},CONVERT_SLIPPAGE_TOLERANCE:1,CONNECT_LABELS:{MOBILE:`Open and continue in a new browser tab`},DEFAULT_FEATURES:{swaps:!0,onramp:!0,receive:!0,send:!0,email:!0,emailShowWallets:!0,socials:[`google`,`x`,`discord`,`farcaster`,`github`,`apple`,`facebook`],connectorTypeOrder:[`walletConnect`,`recent`,`injected`,`featured`,`custom`,`external`,`recommended`],history:!0,analytics:!0,allWallets:!0,legalCheckbox:!1,smartSessions:!1,collapseWallets:!1,walletFeaturesOrder:[`onramp`,`swaps`,`receive`,`send`],connectMethodsOrder:void 0},DEFAULT_ACCOUNT_TYPES:{bip122:`payment`,eip155:`smartAccount`,polkadot:`eoa`,solana:`eoa`},ADAPTER_TYPES:{UNIVERSAL:`universal`,SOLANA:`solana`,WAGMI:`wagmi`,ETHERS:`ethers`,ETHERS5:`ethers5`,BITCOIN:`bitcoin`}},k={cacheExpiry:{portfolio:3e4,nativeBalance:3e4,ens:3e5,identity:3e5},isCacheExpired(e,t){return Date.now()-e>t},getActiveNetworkProps(){let e=k.getActiveNamespace(),t=k.getActiveCaipNetworkId(),n=t?t.split(`:`)[1]:void 0;return{namespace:e,caipNetworkId:t,chainId:n?isNaN(Number(n))?n:Number(n):void 0}},setWalletConnectDeepLink({name:e,href:t}){try{T.setItem(w.DEEPLINK_CHOICE,JSON.stringify({href:t,name:e}))}catch{console.info(`Unable to set WalletConnect deep link`)}},getWalletConnectDeepLink(){try{let e=T.getItem(w.DEEPLINK_CHOICE);if(e)return JSON.parse(e)}catch{console.info(`Unable to get WalletConnect deep link`)}},deleteWalletConnectDeepLink(){try{T.removeItem(w.DEEPLINK_CHOICE)}catch{console.info(`Unable to delete WalletConnect deep link`)}},setActiveNamespace(e){try{T.setItem(w.ACTIVE_NAMESPACE,e)}catch{console.info(`Unable to set active namespace`)}},setActiveCaipNetworkId(e){try{T.setItem(w.ACTIVE_CAIP_NETWORK_ID,e),k.setActiveNamespace(e.split(`:`)[0])}catch{console.info(`Unable to set active caip network id`)}},getActiveCaipNetworkId(){try{return T.getItem(w.ACTIVE_CAIP_NETWORK_ID)}catch{console.info(`Unable to get active caip network id`);return}},deleteActiveCaipNetworkId(){try{T.removeItem(w.ACTIVE_CAIP_NETWORK_ID)}catch{console.info(`Unable to delete active caip network id`)}},deleteConnectedConnectorId(e){try{let t=Pn(e);T.removeItem(t)}catch{console.info(`Unable to delete connected connector id`)}},setAppKitRecent(e){try{let t=k.getRecentWallets();t.find(t=>t.id===e.id)||(t.unshift(e),t.length>2&&t.pop(),T.setItem(w.RECENT_WALLETS,JSON.stringify(t)))}catch{console.info(`Unable to set AppKit recent`)}},getRecentWallets(){try{let e=T.getItem(w.RECENT_WALLETS);return e?JSON.parse(e):[]}catch{console.info(`Unable to get AppKit recent`)}return[]},setConnectedConnectorId(e,t){try{let n=Pn(e);T.setItem(n,t)}catch{console.info(`Unable to set Connected Connector Id`)}},getActiveNamespace(){try{return T.getItem(w.ACTIVE_NAMESPACE)}catch{console.info(`Unable to get active namespace`)}},getConnectedConnectorId(e){if(e)try{let t=Pn(e);return T.getItem(t)}catch{console.info(`Unable to get connected connector id in namespace `,e)}},setConnectedSocialProvider(e){try{T.setItem(w.CONNECTED_SOCIAL,e)}catch{console.info(`Unable to set connected social provider`)}},getConnectedSocialProvider(){try{return T.getItem(w.CONNECTED_SOCIAL)}catch{console.info(`Unable to get connected social provider`)}},deleteConnectedSocialProvider(){try{T.removeItem(w.CONNECTED_SOCIAL)}catch{console.info(`Unable to delete connected social provider`)}},getConnectedSocialUsername(){try{return T.getItem(w.CONNECTED_SOCIAL_USERNAME)}catch{console.info(`Unable to get connected social username`)}},getStoredActiveCaipNetworkId(){return T.getItem(w.ACTIVE_CAIP_NETWORK_ID)?.split(`:`)?.[1]},setConnectionStatus(e){try{T.setItem(w.CONNECTION_STATUS,e)}catch{console.info(`Unable to set connection status`)}},getConnectionStatus(){try{return T.getItem(w.CONNECTION_STATUS)}catch{return}},getConnectedNamespaces(){try{let e=T.getItem(w.CONNECTED_NAMESPACES);return e?.length?e.split(`,`):[]}catch{return[]}},setConnectedNamespaces(e){try{let t=Array.from(new Set(e));T.setItem(w.CONNECTED_NAMESPACES,t.join(`,`))}catch{console.info(`Unable to set namespaces in storage`)}},addConnectedNamespace(e){try{let t=k.getConnectedNamespaces();t.includes(e)||(t.push(e),k.setConnectedNamespaces(t))}catch{console.info(`Unable to add connected namespace`)}},removeConnectedNamespace(e){try{let t=k.getConnectedNamespaces(),n=t.indexOf(e);n>-1&&(t.splice(n,1),k.setConnectedNamespaces(t))}catch{console.info(`Unable to remove connected namespace`)}},getTelegramSocialProvider(){try{return T.getItem(w.TELEGRAM_SOCIAL_PROVIDER)}catch{return console.info(`Unable to get telegram social provider`),null}},setTelegramSocialProvider(e){try{T.setItem(w.TELEGRAM_SOCIAL_PROVIDER,e)}catch{console.info(`Unable to set telegram social provider`)}},removeTelegramSocialProvider(){try{T.removeItem(w.TELEGRAM_SOCIAL_PROVIDER)}catch{console.info(`Unable to remove telegram social provider`)}},getBalanceCache(){let e={};try{let t=T.getItem(w.PORTFOLIO_CACHE);e=t?JSON.parse(t):{}}catch{console.info(`Unable to get balance cache`)}return e},removeAddressFromBalanceCache(e){try{let t=k.getBalanceCache();T.setItem(w.PORTFOLIO_CACHE,JSON.stringify({...t,[e]:void 0}))}catch{console.info(`Unable to remove address from balance cache`,e)}},getBalanceCacheForCaipAddress(e){try{let t=k.getBalanceCache()[e];if(t&&!this.isCacheExpired(t.timestamp,this.cacheExpiry.portfolio))return t.balance;k.removeAddressFromBalanceCache(e)}catch{console.info(`Unable to get balance cache for address`,e)}},updateBalanceCache(e){try{let t=k.getBalanceCache();t[e.caipAddress]=e,T.setItem(w.PORTFOLIO_CACHE,JSON.stringify(t))}catch{console.info(`Unable to update balance cache`,e)}},getNativeBalanceCache(){let e={};try{let t=T.getItem(w.NATIVE_BALANCE_CACHE);e=t?JSON.parse(t):{}}catch{console.info(`Unable to get balance cache`)}return e},removeAddressFromNativeBalanceCache(e){try{let t=k.getBalanceCache();T.setItem(w.NATIVE_BALANCE_CACHE,JSON.stringify({...t,[e]:void 0}))}catch{console.info(`Unable to remove address from balance cache`,e)}},getNativeBalanceCacheForCaipAddress(e){try{let t=k.getNativeBalanceCache()[e];if(t&&!this.isCacheExpired(t.timestamp,this.cacheExpiry.nativeBalance))return t;console.info(`Discarding cache for address`,e),k.removeAddressFromBalanceCache(e)}catch{console.info(`Unable to get balance cache for address`,e)}},updateNativeBalanceCache(e){try{let t=k.getNativeBalanceCache();t[e.caipAddress]=e,T.setItem(w.NATIVE_BALANCE_CACHE,JSON.stringify(t))}catch{console.info(`Unable to update balance cache`,e)}},getEnsCache(){let e={};try{let t=T.getItem(w.ENS_CACHE);e=t?JSON.parse(t):{}}catch{console.info(`Unable to get ens name cache`)}return e},getEnsFromCacheForAddress(e){try{let t=k.getEnsCache()[e];if(t&&!this.isCacheExpired(t.timestamp,this.cacheExpiry.ens))return t.ens;k.removeEnsFromCache(e)}catch{console.info(`Unable to get ens name from cache`,e)}},updateEnsCache(e){try{let t=k.getEnsCache();t[e.address]=e,T.setItem(w.ENS_CACHE,JSON.stringify(t))}catch{console.info(`Unable to update ens name cache`,e)}},removeEnsFromCache(e){try{let t=k.getEnsCache();T.setItem(w.ENS_CACHE,JSON.stringify({...t,[e]:void 0}))}catch{console.info(`Unable to remove ens name from cache`,e)}},getIdentityCache(){let e={};try{let t=T.getItem(w.IDENTITY_CACHE);e=t?JSON.parse(t):{}}catch{console.info(`Unable to get identity cache`)}return e},getIdentityFromCacheForAddress(e){try{let t=k.getIdentityCache()[e];if(t&&!this.isCacheExpired(t.timestamp,this.cacheExpiry.identity))return t.identity;k.removeIdentityFromCache(e)}catch{console.info(`Unable to get identity from cache`,e)}},updateIdentityCache(e){try{let t=k.getIdentityCache();t[e.address]={identity:e.identity,timestamp:e.timestamp},T.setItem(w.IDENTITY_CACHE,JSON.stringify(t))}catch{console.info(`Unable to update identity cache`,e)}},removeIdentityFromCache(e){try{let t=k.getIdentityCache();T.setItem(w.IDENTITY_CACHE,JSON.stringify({...t,[e]:void 0}))}catch{console.info(`Unable to remove identity from cache`,e)}},clearAddressCache(){try{T.removeItem(w.PORTFOLIO_CACHE),T.removeItem(w.NATIVE_BALANCE_CACHE),T.removeItem(w.ENS_CACHE),T.removeItem(w.IDENTITY_CACHE)}catch{console.info(`Unable to clear address cache`)}}},A={isMobile(){return this.isClient()?!!(window?.matchMedia(`(pointer:coarse)`)?.matches||/Android|webOS|iPhone|iPad|iPod|BlackBerry|Opera Mini/u.test(navigator.userAgent)):!1},checkCaipNetwork(e,t=``){return e?.caipNetworkId.toLocaleLowerCase().includes(t.toLowerCase())},isAndroid(){if(!this.isMobile())return!1;let e=window?.navigator.userAgent.toLowerCase();return A.isMobile()&&e.includes(`android`)},isIos(){if(!this.isMobile())return!1;let e=window?.navigator.userAgent.toLowerCase();return e.includes(`iphone`)||e.includes(`ipad`)},isSafari(){return this.isClient()?(window?.navigator.userAgent.toLowerCase()).includes(`safari`):!1},isClient(){return typeof window<`u`},isPairingExpired(e){return e?e-Date.now()<=O.TEN_SEC_MS:!0},isAllowedRetry(e,t=O.ONE_SEC_MS){return Date.now()-e>=t},copyToClopboard(e){navigator.clipboard.writeText(e)},isIframe(){try{return window?.self!==window?.top}catch{return!1}},getPairingExpiry(){return Date.now()+O.FOUR_MINUTES_MS},getNetworkId(e){return e?.split(`:`)[1]},getPlainAddress(e){return e?.split(`:`)[2]},async wait(e){return new Promise(t=>{setTimeout(t,e)})},debounce(e,t=500){let n;return(...r)=>{function i(){e(...r)}n&&clearTimeout(n),n=setTimeout(i,t)}},isHttpUrl(e){return e.startsWith(`http://`)||e.startsWith(`https://`)},formatNativeUrl(e,t){if(A.isHttpUrl(e))return this.formatUniversalUrl(e,t);let n=e;return n.includes(`://`)||(n=e.replaceAll(`/`,``).replaceAll(`:`,``),n=`${n}://`),n.endsWith(`/`)||(n=`${n}/`),this.isTelegram()&&this.isAndroid()&&(t=encodeURIComponent(t)),{redirect:`${n}wc?uri=${encodeURIComponent(t)}`,href:n}},formatUniversalUrl(e,t){if(!A.isHttpUrl(e))return this.formatNativeUrl(e,t);let n=e;return n.endsWith(`/`)||(n=`${n}/`),{redirect:`${n}wc?uri=${encodeURIComponent(t)}`,href:n}},getOpenTargetForPlatform(e){return e===`popupWindow`?e:this.isTelegram()?k.getTelegramSocialProvider()?`_top`:`_blank`:e},openHref(e,t,n){window?.open(e,this.getOpenTargetForPlatform(t),n||`noreferrer noopener`)},returnOpenHref(e,t,n){return window?.open(e,this.getOpenTargetForPlatform(t),n||`noreferrer noopener`)},isTelegram(){return typeof window<`u`&&(!!window.TelegramWebviewProxy||!!window.Telegram||!!window.TelegramWebviewProxyProto)},async preloadImage(e){let t=new Promise((t,n)=>{let r=new Image;r.onload=t,r.onerror=n,r.crossOrigin=`anonymous`,r.src=e});return Promise.race([t,A.wait(2e3)])},formatBalance(e,t){let n=`0.000`;if(typeof e==`string`){let t=Number(e);if(t){let e=Math.floor(t*1e3)/1e3;e&&(n=e.toString())}}return`${n}${t?` ${t}`:``}`},formatBalance2(e,t){let n;if(e===`0`)n=`0`;else if(typeof e==`string`){let t=Number(e);t&&(n=t.toString().match(/^-?\d+(?:\.\d{0,3})?/u)?.[0])}return{value:n??`0`,rest:n===`0`?`000`:``,symbol:t}},getApiUrl(){return C.W3M_API_URL},getBlockchainApiUrl(){return C.BLOCKCHAIN_API_RPC_URL},getAnalyticsUrl(){return C.PULSE_API_URL},getUUID(){return crypto?.randomUUID?crypto.randomUUID():`xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`.replace(/[xy]/gu,e=>{let t=Math.random()*16|0;return(e===`x`?t:t&3|8).toString(16)})},parseError(e){return typeof e==`string`?e:typeof e?.issues?.[0]?.message==`string`?e.issues[0].message:e instanceof Error?e.message:`Unknown error`},sortRequestedNetworks(e,t=[]){let n={};return t&&e&&(e.forEach((e,t)=>{n[e]=t}),t.sort((e,t)=>{let r=n[e.id],i=n[t.id];return r!==void 0&&i!==void 0?r-i:r===void 0?i===void 0?0:1:-1})),t},calculateBalance(e){let t=0;for(let n of e)t+=n.value??0;return t},formatTokenBalance(e){let[t,n]=e.toFixed(2).split(`.`);return{dollars:t,pennies:n}},isAddress(e,t=`eip155`){switch(t){case`eip155`:return/^(?:0x)?[0-9a-f]{40}$/iu.test(e)?!!(/^(?:0x)?[0-9a-f]{40}$/iu.test(e)||/^(?:0x)?[0-9A-F]{40}$/iu.test(e)):!1;case`solana`:return/[1-9A-HJ-NP-Za-km-z]{32,44}$/iu.test(e);default:return!1}},uniqueBy(e,t){let n=new Set;return e.filter(e=>{let r=e[t];return n.has(r)?!1:(n.add(r),!0)})},generateSdkVersion(e,t,n){return`${t}-${e.length===0?O.ADAPTER_TYPES.UNIVERSAL:e.map(e=>e.adapterType).join(`,`)}-${n}`},createAccount(e,t,n,r,i){return{namespace:e,address:t,type:n,publicKey:r,path:i}},isCaipAddress(e){if(typeof e!=`string`)return!1;let t=e.split(`:`),n=t[0];return t.filter(Boolean).length===3&&n in C.CHAIN_NAME_MAP},isMac(){let e=window?.navigator.userAgent.toLowerCase();return e.includes(`macintosh`)&&!e.includes(`safari`)},formatTelegramSocialLoginUrl(e){let t=`--${encodeURIComponent(window?.location.href)}`,n=`state=`;if(new URL(e).host===`auth.magic.link`){let r=e.substring(e.indexOf(`provider_authorization_url=`)+27),i=this.injectIntoUrl(decodeURIComponent(r),n,t);return e.replace(r,encodeURIComponent(i))}return this.injectIntoUrl(e,n,t)},injectIntoUrl(e,t,n){let r=e.indexOf(t);if(r===-1)throw Error(`${t} parameter not found in the URL: ${e}`);let i=e.indexOf(`&`,r),a=t.length,o=i===-1?e.length:i,s=e.substring(0,r+a),c=e.substring(r+a,o),l=e.substring(i);return s+(c+n)+l}};async function sr(...e){let t=await fetch(...e);if(!t.ok)throw Error(`HTTP status code: ${t.status}`,{cause:t});return t}var cr=class{constructor({baseUrl:e,clientId:t}){this.baseUrl=e,this.clientId=t}async get({headers:e,signal:t,cache:n,...r}){return(await sr(this.createUrl(r),{method:`GET`,headers:e,signal:t,cache:n})).json()}async getBlob({headers:e,signal:t,...n}){return(await sr(this.createUrl(n),{method:`GET`,headers:e,signal:t})).blob()}async post({body:e,headers:t,signal:n,...r}){return(await sr(this.createUrl(r),{method:`POST`,headers:t,body:e?JSON.stringify(e):void 0,signal:n})).json()}async put({body:e,headers:t,signal:n,...r}){return(await sr(this.createUrl(r),{method:`PUT`,headers:t,body:e?JSON.stringify(e):void 0,signal:n})).json()}async delete({body:e,headers:t,signal:n,...r}){return(await sr(this.createUrl(r),{method:`DELETE`,headers:t,body:e?JSON.stringify(e):void 0,signal:n})).json()}createUrl({path:e,params:t}){let n=new URL(e,this.baseUrl);return t&&Object.entries(t).forEach(([e,t])=>{t&&n.searchParams.append(e,t)}),this.clientId&&n.searchParams.append(`clientId`,this.clientId),n}},lr={handleSolanaDeeplinkRedirect(e){if(J.state.activeChain===C.CHAIN.SOLANA){let t=window.location.href,n=encodeURIComponent(t);if(e===`Phantom`&&!(`phantom`in window)){let e=t.startsWith(`https`)?`https`:`http`,r=t.split(`/`)[2],i=encodeURIComponent(`${e}://${r}`);window.location.href=`https://phantom.app/ul/browse/${n}?ref=${i}`}e===`Coinbase Wallet`&&!(`coinbaseSolana`in window)&&(window.location.href=`https://go.cb-w.com/dapp?cb_url=${n}`)}}},ur=E({walletImages:{},networkImages:{},chainImages:{},connectorImages:{},tokenImages:{},currencyImages:{}}),dr={state:ur,subscribeNetworkImages(e){return D(ur.networkImages,()=>e(ur.networkImages))},subscribeKey(e,t){return ir(ur,e,t)},subscribe(e){return D(ur,()=>e(ur))},setWalletImage(e,t){ur.walletImages[e]=t},setNetworkImage(e,t){ur.networkImages[e]=t},setChainImage(e,t){ur.chainImages[e]=t},setConnectorImage(e,t){ur.connectorImages={...ur.connectorImages,[e]:t}},setTokenImage(e,t){ur.tokenImages[e]=t},setCurrencyImage(e,t){ur.currencyImages[e]=t}},fr={eip155:`ba0ba0cd-17c6-4806-ad93-f9d174f17900`,solana:`a1b58899-f671-4276-6a5e-56ca5bd59700`,polkadot:``,bip122:`0b4838db-0161-4ffe-022d-532bf03dba00`},pr=E({networkImagePromises:{}}),mr={async fetchWalletImage(e){if(e)return await I._fetchWalletImage(e),this.getWalletImageById(e)},async fetchNetworkImage(e){return e?this.getNetworkImageById(e)||(pr.networkImagePromises[e]||(pr.networkImagePromises[e]=I._fetchNetworkImage(e)),await pr.networkImagePromises[e],this.getNetworkImageById(e)):void 0},getWalletImageById(e){if(e)return dr.state.walletImages[e]},getWalletImage(e){if(e?.image_url)return e?.image_url;if(e?.image_id)return dr.state.walletImages[e.image_id]},getNetworkImage(e){if(e?.assets?.imageUrl)return e?.assets?.imageUrl;if(e?.assets?.imageId)return dr.state.networkImages[e.assets.imageId]},getNetworkImageById(e){if(e)return dr.state.networkImages[e]},getConnectorImage(e){if(e?.imageUrl)return e.imageUrl;if(e?.imageId)return dr.state.connectorImages[e.imageId]},getChainImage(e){return dr.state.networkImages[fr[e]]}},hr={getFeatureValue(e,t){let n=t?.[e];return n===void 0?O.DEFAULT_FEATURES[e]:n},filterSocialsByPlatform(e){if(!e||!e.length)return e;if(A.isTelegram()){if(A.isIos())return e.filter(e=>e!==`google`);if(A.isMac())return e.filter(e=>e!==`x`);if(A.isAndroid())return e.filter(e=>![`facebook`,`x`].includes(e))}return e}},j=E({features:O.DEFAULT_FEATURES,projectId:``,sdkType:`appkit`,sdkVersion:`html-wagmi-undefined`,defaultAccountTypes:O.DEFAULT_ACCOUNT_TYPES,enableNetworkSwitch:!0}),M={state:j,subscribeKey(e,t){return ir(j,e,t)},setOptions(e){Object.assign(j,e)},setFeatures(e){e&&(j.features||=O.DEFAULT_FEATURES,j.features={...j.features,...e},j.features.socials&&(j.features.socials=hr.filterSocialsByPlatform(j.features.socials)))},setProjectId(e){j.projectId=e},setCustomRpcUrls(e){j.customRpcUrls=e},setAllWallets(e){j.allWallets=e},setIncludeWalletIds(e){j.includeWalletIds=e},setExcludeWalletIds(e){j.excludeWalletIds=e},setFeaturedWalletIds(e){j.featuredWalletIds=e},setTokens(e){j.tokens=e},setTermsConditionsUrl(e){j.termsConditionsUrl=e},setPrivacyPolicyUrl(e){j.privacyPolicyUrl=e},setCustomWallets(e){j.customWallets=e},setIsSiweEnabled(e){j.isSiweEnabled=e},setIsUniversalProvider(e){j.isUniversalProvider=e},setSdkVersion(e){j.sdkVersion=e},setMetadata(e){j.metadata=e},setDisableAppend(e){j.disableAppend=e},setEIP6963Enabled(e){j.enableEIP6963=e},setDebug(e){j.debug=e},setEnableWalletConnect(e){j.enableWalletConnect=e},setEnableWalletGuide(e){j.enableWalletGuide=e},setEnableAuthLogger(e){j.enableAuthLogger=e},setEnableWallets(e){j.enableWallets=e},setHasMultipleAddresses(e){j.hasMultipleAddresses=e},setSIWX(e){j.siwx=e},setConnectMethodsOrder(e){j.features={...j.features,connectMethodsOrder:e}},setWalletFeaturesOrder(e){j.features={...j.features,walletFeaturesOrder:e}},setSocialsOrder(e){j.features={...j.features,socials:e}},setCollapseWallets(e){j.features={...j.features,collapseWallets:e}},setEnableEmbedded(e){j.enableEmbedded=e},setAllowUnsupportedChain(e){j.allowUnsupportedChain=e},setManualWCControl(e){j.manualWCControl=e},setEnableNetworkSwitch(e){j.enableNetworkSwitch=e},setDefaultAccountTypes(e={}){Object.entries(e).forEach(([e,t])=>{t&&(j.defaultAccountTypes[e]=t)})},setUniversalProviderConfigOverride(e){j.universalProviderConfigOverride=e},getUniversalProviderConfigOverride(){return j.universalProviderConfigOverride},getSnapshot(){return nr(j)}},gr=E({message:``,variant:`info`,open:!1}),_r={state:gr,subscribeKey(e,t){return ir(gr,e,t)},open(e,t){let{debug:n}=M.state,{shortMessage:r,longMessage:i}=e;n&&(gr.message=r,gr.variant=t,gr.open=!0),i&&console.error(typeof i==`function`?i():i)},close(){gr.open=!1,gr.message=``,gr.variant=`info`}},vr=new cr({baseUrl:A.getAnalyticsUrl(),clientId:null}),yr=[`MODAL_CREATED`],br=E({timestamp:Date.now(),reportedErrors:{},data:{type:`track`,event:`MODAL_CREATED`}}),N={state:br,subscribe(e){return D(br,()=>e(br))},getSdkProperties(){let{projectId:e,sdkType:t,sdkVersion:n}=M.state;return{projectId:e,st:t,sv:n||`html-wagmi-4.2.2`}},async _sendAnalyticsEvent(e){try{let t=Z.state.address;if(yr.includes(e.data.event)||typeof window>`u`)return;await vr.post({path:`/e`,params:N.getSdkProperties(),body:{eventId:A.getUUID(),url:window.location.href,domain:window.location.hostname,timestamp:e.timestamp,props:{...e.data,address:t}}}),br.reportedErrors.FORBIDDEN=!1}catch(e){e instanceof Error&&e.cause instanceof Response&&e.cause.status===C.HTTP_STATUS_CODES.FORBIDDEN&&!br.reportedErrors.FORBIDDEN&&(_r.open({shortMessage:`Invalid App Configuration`,longMessage:`Origin ${Fn()?window.origin:`uknown`} not found on Allowlist - update configuration on cloud.reown.com`},`error`),br.reportedErrors.FORBIDDEN=!0)}},sendEvent(e){br.timestamp=Date.now(),br.data=e,M.state.features?.analytics&&N._sendAnalyticsEvent(br)}},P=new cr({baseUrl:A.getApiUrl(),clientId:null}),xr=`40`,Sr=`4`,Cr=20,F=E({promises:{},page:1,count:0,featured:[],allFeatured:[],recommended:[],allRecommended:[],wallets:[],search:[],isAnalyticsEnabled:!1,excludedWallets:[],isFetchingRecommendedWallets:!1}),I={state:F,subscribeKey(e,t){return ir(F,e,t)},_getSdkProperties(){let{projectId:e,sdkType:t,sdkVersion:n}=M.state;return{projectId:e,st:t||`appkit`,sv:n||`html-wagmi-4.2.2`}},_filterOutExtensions(e){return M.state.isUniversalProvider?e.filter(e=>!!(e.mobile_link||e.desktop_link||e.webapp_link)):e},async _fetchWalletImage(e){let t=`${P.baseUrl}/getWalletImage/${e}`,n=await P.getBlob({path:t,params:I._getSdkProperties()});dr.setWalletImage(e,URL.createObjectURL(n))},async _fetchNetworkImage(e){let t=`${P.baseUrl}/public/getAssetImage/${e}`,n=await P.getBlob({path:t,params:I._getSdkProperties()});dr.setNetworkImage(e,URL.createObjectURL(n))},async _fetchConnectorImage(e){let t=`${P.baseUrl}/public/getAssetImage/${e}`,n=await P.getBlob({path:t,params:I._getSdkProperties()});dr.setConnectorImage(e,URL.createObjectURL(n))},async _fetchCurrencyImage(e){let t=`${P.baseUrl}/public/getCurrencyImage/${e}`,n=await P.getBlob({path:t,params:I._getSdkProperties()});dr.setCurrencyImage(e,URL.createObjectURL(n))},async _fetchTokenImage(e){let t=`${P.baseUrl}/public/getTokenImage/${e}`,n=await P.getBlob({path:t,params:I._getSdkProperties()});dr.setTokenImage(e,URL.createObjectURL(n))},async fetchNetworkImages(){let e=J.getAllRequestedCaipNetworks()?.map(({assets:e})=>e?.imageId).filter(Boolean).filter(e=>!mr.getNetworkImageById(e));e&&await Promise.allSettled(e.map(e=>I._fetchNetworkImage(e)))},async fetchConnectorImages(){let{connectors:e}=B.state,t=e.map(({imageId:e})=>e).filter(Boolean);await Promise.allSettled(t.map(e=>I._fetchConnectorImage(e)))},async fetchCurrencyImages(e=[]){await Promise.allSettled(e.map(e=>I._fetchCurrencyImage(e)))},async fetchTokenImages(e=[]){await Promise.allSettled(e.map(e=>I._fetchTokenImage(e)))},async fetchFeaturedWallets(){let{featuredWalletIds:e}=M.state;if(e?.length){let{data:t}=await P.get({path:`/getWallets`,params:{...I._getSdkProperties(),page:`1`,entries:e?.length?String(e.length):Sr,include:e?.join(`,`)}});t.sort((t,n)=>e.indexOf(t.id)-e.indexOf(n.id));let n=t.map(e=>e.image_id).filter(Boolean);await Promise.allSettled(n.map(e=>I._fetchWalletImage(e))),F.featured=t,F.allFeatured=t}},async fetchRecommendedWallets(){try{F.isFetchingRecommendedWallets=!0;let{includeWalletIds:e,excludeWalletIds:t,featuredWalletIds:n}=M.state,r=[...t??[],...n??[]].filter(Boolean),i=J.getRequestedCaipNetworkIds().join(`,`),{data:a,count:o}=await P.get({path:`/getWallets`,params:{...I._getSdkProperties(),page:`1`,chains:i,entries:Sr,include:e?.join(`,`),exclude:r?.join(`,`)}}),s=k.getRecentWallets(),c=a.map(e=>e.image_id).filter(Boolean),l=s.map(e=>e.image_id).filter(Boolean);await Promise.allSettled([...c,...l].map(e=>I._fetchWalletImage(e))),F.recommended=a,F.allRecommended=a,F.count=o??0}catch{}finally{F.isFetchingRecommendedWallets=!1}},async fetchWallets({page:e}){let{includeWalletIds:t,excludeWalletIds:n,featuredWalletIds:r}=M.state,i=J.getRequestedCaipNetworkIds().join(`,`),a=[...F.recommended.map(({id:e})=>e),...n??[],...r??[]].filter(Boolean),{data:o,count:s}=await P.get({path:`/getWallets`,params:{...I._getSdkProperties(),page:String(e),entries:xr,chains:i,include:t?.join(`,`),exclude:a.join(`,`)}}),c=o.slice(0,Cr).map(e=>e.image_id).filter(Boolean);await Promise.allSettled(c.map(e=>I._fetchWalletImage(e))),F.wallets=A.uniqueBy([...F.wallets,...I._filterOutExtensions(o)],`id`),F.count=s>F.count?s:F.count,F.page=e},async initializeExcludedWallets({ids:e}){let t=J.getRequestedCaipNetworkIds().join(`,`),{data:n}=await P.get({path:`/getWallets`,params:{...I._getSdkProperties(),page:`1`,entries:String(e.length),chains:t,include:e?.join(`,`)}});n&&n.forEach(e=>{e?.rdns&&F.excludedWallets.push({rdns:e.rdns,name:e.name})})},async searchWallet({search:e,badge:t}){let{includeWalletIds:n,excludeWalletIds:r}=M.state;F.search=[];let i=J.getRequestedCaipNetworkIds().join(`,`),{data:a}=await P.get({path:`/getWallets`,params:{...I._getSdkProperties(),page:`1`,entries:`100`,search:e?.trim(),badge_type:t,chains:i,include:n?.join(`,`),exclude:r?.join(`,`)}});N.sendEvent({type:`track`,event:`SEARCH_WALLET`,properties:{badge:t??``,search:e??``}});let o=a.map(e=>e.image_id).filter(Boolean);await Promise.allSettled([...o.map(e=>I._fetchWalletImage(e)),A.wait(300)]),F.search=I._filterOutExtensions(a)},initPromise(e,t){return F.promises[e]||(F.promises[e]=t())},prefetch({fetchConnectorImages:e=!0,fetchFeaturedWallets:t=!0,fetchRecommendedWallets:n=!0,fetchNetworkImages:r=!0}={}){let i=[e&&I.initPromise(`connectorImages`,I.fetchConnectorImages),t&&I.initPromise(`featuredWallets`,I.fetchFeaturedWallets),n&&I.initPromise(`recommendedWallets`,I.fetchRecommendedWallets),r&&I.initPromise(`networkImages`,I.fetchNetworkImages)].filter(Boolean);return Promise.allSettled(i)},prefetchAnalyticsConfig(){M.state.features?.analytics&&I.fetchAnalyticsConfig()},async fetchAnalyticsConfig(){try{let{isAnalyticsEnabled:e}=await P.get({path:`/getAnalyticsConfig`,params:I._getSdkProperties()});M.setFeatures({analytics:e})}catch{M.setFeatures({analytics:!1})}},setFilterByNamespace(e){if(!e){F.featured=F.allFeatured,F.recommended=F.allRecommended;return}let t=J.getRequestedCaipNetworkIds().join(`,`);F.featured=F.allFeatured.filter(e=>e.chains?.some(e=>t.includes(e))),F.recommended=F.allRecommended.filter(e=>e.chains?.some(e=>t.includes(e)))}},L=E({view:`Connect`,history:[`Connect`],transactionStack:[]}),R={state:L,subscribeKey(e,t){return ir(L,e,t)},pushTransactionStack(e){L.transactionStack.push(e)},popTransactionStack(e){let t=L.transactionStack.pop();if(t)if(e)this.goBack(),t?.onCancel?.();else{if(t.goBack)this.goBack();else if(t.replace){let e=L.history.indexOf(`ConnectingSiwe`);e>0?this.goBackToIndex(e-1):($.close(),L.history=[])}else t.view&&this.reset(t.view);t?.onSuccess?.()}},push(e,t){e!==L.view&&(L.view=e,L.history.push(e),L.data=t)},reset(e,t){L.view=e,L.history=[e],L.data=t},replace(e,t){L.history.at(-1)!==e&&(L.view=e,L.history[L.history.length-1]=e,L.data=t)},goBack(){let e=!J.state.activeCaipAddress&&this.state.view===`ConnectingFarcaster`;if(L.history.length>1&&!L.history.includes(`UnsupportedChain`)){L.history.pop();let[e]=L.history.slice(-1);e&&(L.view=e)}else $.close();L.data?.wallet&&(L.data.wallet=void 0),setTimeout(()=>{if(e){Z.setFarcasterUrl(void 0,J.state.activeChain);let e=B.getAuthConnector();e?.provider?.reload();let t=nr(M.state);e?.provider?.syncDappData?.({metadata:t.metadata,sdkVersion:t.sdkVersion,projectId:t.projectId,sdkType:t.sdkType})}},100)},goBackToIndex(e){if(L.history.length>1){L.history=L.history.slice(0,e+1);let[t]=L.history.slice(-1);t&&(L.view=t)}}},wr=E({themeMode:`dark`,themeVariables:{},w3mThemeVariables:void 0}),Tr={state:wr,subscribe(e){return D(wr,()=>e(wr))},setThemeMode(e){wr.themeMode=e;try{let t=B.getAuthConnector();if(t){let n=Tr.getSnapshot().themeVariables;t.provider.syncTheme({themeMode:e,themeVariables:n,w3mThemeVariables:In(n,e)})}}catch{console.info(`Unable to sync theme to auth connector`)}},setThemeVariables(e){wr.themeVariables={...wr.themeVariables,...e};try{let e=B.getAuthConnector();if(e){let t=Tr.getSnapshot().themeVariables;e.provider.syncTheme({themeVariables:t,w3mThemeVariables:In(wr.themeVariables,wr.themeMode)})}}catch{console.info(`Unable to sync theme to auth connector`)}},getSnapshot(){return nr(wr)}},Er={eip155:void 0,solana:void 0,polkadot:void 0,bip122:void 0},z=E({allConnectors:[],connectors:[],activeConnector:void 0,filterByNamespace:void 0,activeConnectorIds:{...Er}}),B={state:z,subscribe(e){return D(z,()=>{e(z)})},subscribeKey(e,t){return ir(z,e,t)},initialize(e){e.forEach(e=>{let t=k.getConnectedConnectorId(e);t&&this.setConnectorId(t,e)})},setActiveConnector(e){e&&(z.activeConnector=rr(e))},setConnectors(e){e.filter(e=>!z.allConnectors.some(t=>t.id===e.id&&this.getConnectorName(t.name)===this.getConnectorName(e.name)&&t.chain===e.chain)).forEach(e=>{e.type!==`MULTI_CHAIN`&&z.allConnectors.push(rr(e))}),z.connectors=this.mergeMultiChainConnectors(z.allConnectors)},removeAdapter(e){z.allConnectors=z.allConnectors.filter(t=>t.chain!==e),z.connectors=this.mergeMultiChainConnectors(z.allConnectors)},mergeMultiChainConnectors(e){let t=this.generateConnectorMapByName(e),n=[];return t.forEach(e=>{let t=e[0],r=t?.id===C.CONNECTOR_ID.AUTH;e.length>1&&t?n.push({name:t.name,imageUrl:t.imageUrl,imageId:t.imageId,connectors:[...e],type:r?`AUTH`:`MULTI_CHAIN`,chain:`eip155`,id:t?.id||``}):t&&n.push(t)}),n},generateConnectorMapByName(e){let t=new Map;return e.forEach(e=>{let{name:n}=e,r=this.getConnectorName(n);if(!r)return;let i=t.get(r)||[];i.find(t=>t.chain===e.chain)||i.push(e),t.set(r,i)}),t},getConnectorName(e){return e&&({"Trust Wallet":`Trust`}[e]||e)},getUniqueConnectorsByName(e){let t=[];return e.forEach(e=>{t.find(t=>t.chain===e.chain)||t.push(e)}),t},addConnector(e){if(e.id===C.CONNECTOR_ID.AUTH){let t=e,n=nr(M.state),r=Tr.getSnapshot().themeMode,i=Tr.getSnapshot().themeVariables;t?.provider?.syncDappData?.({metadata:n.metadata,sdkVersion:n.sdkVersion,projectId:n.projectId,sdkType:n.sdkType}),t?.provider?.syncTheme({themeMode:r,themeVariables:i,w3mThemeVariables:In(i,r)}),this.setConnectors([e])}else this.setConnectors([e])},getAuthConnector(e){let t=e||J.state.activeChain,n=z.connectors.find(e=>e.id===C.CONNECTOR_ID.AUTH);if(n)return n?.connectors?.length?n.connectors.find(e=>e.chain===t):n},getAnnouncedConnectorRdns(){return z.connectors.filter(e=>e.type===`ANNOUNCED`).map(e=>e.info?.rdns)},getConnectorById(e){return z.allConnectors.find(t=>t.id===e)},getConnector(e,t){return z.allConnectors.filter(e=>e.chain===J.state.activeChain).find(n=>n.explorerId===e||n.info?.rdns===t)},syncIfAuthConnector(e){if(e.id!==`ID_AUTH`)return;let t=e,n=nr(M.state),r=Tr.getSnapshot().themeMode,i=Tr.getSnapshot().themeVariables;t?.provider?.syncDappData?.({metadata:n.metadata,sdkVersion:n.sdkVersion,sdkType:n.sdkType,projectId:n.projectId}),t.provider.syncTheme({themeMode:r,themeVariables:i,w3mThemeVariables:In(i,r)})},getConnectorsByNamespace(e){let t=z.allConnectors.filter(t=>t.chain===e);return this.mergeMultiChainConnectors(t)},selectWalletConnector(e){let t=B.getConnector(e.id,e.rdns);J.state.activeChain===C.CHAIN.SOLANA&&lr.handleSolanaDeeplinkRedirect(t?.name||e.name||``),t?R.push(`ConnectingExternal`,{connector:t}):R.push(`ConnectingWalletConnect`,{wallet:e})},getConnectors(e){return e?this.getConnectorsByNamespace(e):this.mergeMultiChainConnectors(z.allConnectors)},setFilterByNamespace(e){z.filterByNamespace=e,z.connectors=this.getConnectors(e),I.setFilterByNamespace(e)},setConnectorId(e,t){e&&(z.activeConnectorIds={...z.activeConnectorIds,[t]:e},k.setConnectedConnectorId(t,e))},removeConnectorId(e){z.activeConnectorIds={...z.activeConnectorIds,[e]:void 0},k.deleteConnectedConnectorId(e)},getConnectorId(e){if(e)return z.activeConnectorIds[e]},isConnected(e){return e?!!z.activeConnectorIds[e]:Object.values(z.activeConnectorIds).some(e=>!!e)},resetConnectorIds(){z.activeConnectorIds={...Er}}};function Dr(e,t){return B.getConnectorId(e)===t}function Or(e){let t=Array.from(J.state.chains.keys()),n=[];return e?(n.push([e,J.state.chains.get(e)]),Dr(e,C.CONNECTOR_ID.WALLET_CONNECT)?t.forEach(t=>{t!==e&&Dr(t,C.CONNECTOR_ID.WALLET_CONNECT)&&n.push([t,J.state.chains.get(t)])}):Dr(e,C.CONNECTOR_ID.AUTH)&&t.forEach(t=>{t!==e&&Dr(t,C.CONNECTOR_ID.AUTH)&&n.push([t,J.state.chains.get(t)])})):n=Array.from(J.state.chains.entries()),n}typeof process<`u`&&{}.NEXT_PUBLIC_SECURE_SITE_SDK_URL,typeof process<`u`&&{}.NEXT_PUBLIC_DEFAULT_LOG_LEVEL,typeof process<`u`&&{}.NEXT_PUBLIC_SECURE_SITE_SDK_VERSION;var kr={SAFE_RPC_METHODS:`eth_accounts.eth_blockNumber.eth_call.eth_chainId.eth_estimateGas.eth_feeHistory.eth_gasPrice.eth_getAccount.eth_getBalance.eth_getBlockByHash.eth_getBlockByNumber.eth_getBlockReceipts.eth_getBlockTransactionCountByHash.eth_getBlockTransactionCountByNumber.eth_getCode.eth_getFilterChanges.eth_getFilterLogs.eth_getLogs.eth_getProof.eth_getStorageAt.eth_getTransactionByBlockHashAndIndex.eth_getTransactionByBlockNumberAndIndex.eth_getTransactionByHash.eth_getTransactionCount.eth_getTransactionReceipt.eth_getUncleCountByBlockHash.eth_getUncleCountByBlockNumber.eth_maxPriorityFeePerGas.eth_newBlockFilter.eth_newFilter.eth_newPendingTransactionFilter.eth_sendRawTransaction.eth_syncing.eth_uninstallFilter.wallet_getCapabilities.wallet_getCallsStatus.eth_getUserOperationReceipt.eth_estimateUserOperationGas.eth_getUserOperationByHash.eth_supportedEntryPoints.wallet_getAssets`.split(`.`),NOT_SAFE_RPC_METHODS:[`personal_sign`,`eth_signTypedData_v4`,`eth_sendTransaction`,`solana_signMessage`,`solana_signTransaction`,`solana_signAllTransactions`,`solana_signAndSendTransaction`,`wallet_sendCalls`,`wallet_grantPermissions`,`wallet_revokePermissions`,`eth_sendUserOperation`],GET_CHAIN_ID:`eth_chainId`,RPC_METHOD_NOT_ALLOWED_MESSAGE:`Requested RPC call is not allowed`,RPC_METHOD_NOT_ALLOWED_UI_MESSAGE:`Action not allowed`,ACCOUNT_TYPES:{EOA:`eoa`,SMART_ACCOUNT:`smartAccount`}},Ar=Object.freeze({message:``,variant:`success`,svg:void 0,open:!1,autoClose:!0}),V=E({...Ar}),H={state:V,subscribeKey(e,t){return ir(V,e,t)},showLoading(e,t={}){this._showMessage({message:e,variant:`loading`,...t})},showSuccess(e){this._showMessage({message:e,variant:`success`})},showSvg(e,t){this._showMessage({message:e,svg:t})},showError(e){let t=A.parseError(e);this._showMessage({message:t,variant:`error`})},hide(){V.message=Ar.message,V.variant=Ar.variant,V.svg=Ar.svg,V.open=Ar.open,V.autoClose=Ar.autoClose},_showMessage({message:e,svg:t,variant:n=`success`,autoClose:r=Ar.autoClose}){V.open?(V.open=!1,setTimeout(()=>{V.message=e,V.variant=n,V.svg=t,V.open=!0,V.autoClose=r},150)):(V.message=e,V.variant=n,V.svg=t,V.open=!0,V.autoClose=r)}},jr={getSIWX(){return M.state.siwx},async initializeIfEnabled(){let e=M.state.siwx,t=J.getActiveCaipAddress();if(!(e&&t))return;let[n,r,i]=t.split(`:`);if(J.checkIfSupportedNetwork(n))try{if((await e.getSessions(`${n}:${r}`,i)).length)return;await $.open({view:`SIWXSignMessage`})}catch(e){console.error(`SIWXUtil:initializeIfEnabled`,e),N.sendEvent({type:`track`,event:`SIWX_AUTH_ERROR`,properties:this.getSIWXEventProperties()}),await G._getClient()?.disconnect().catch(console.error),R.reset(`Connect`),H.showError(`A problem occurred while trying initialize authentication`)}},async requestSignMessage(){let e=M.state.siwx,t=A.getPlainAddress(J.getActiveCaipAddress()),n=J.getActiveCaipNetwork(),r=G._getClient();if(!e)throw Error(`SIWX is not enabled`);if(!t)throw Error(`No ActiveCaipAddress found`);if(!n)throw Error(`No ActiveCaipNetwork or client found`);if(!r)throw Error(`No ConnectionController client found`);try{let i=await e.createMessage({chainId:n.caipNetworkId,accountAddress:t}),a=i.toString();B.getConnectorId(n.chainNamespace)===C.CONNECTOR_ID.AUTH&&R.pushTransactionStack({view:null,goBack:!1,replace:!0});let o=await r.signMessage(a);await e.addSession({data:i,message:a,signature:o}),$.close(),N.sendEvent({type:`track`,event:`SIWX_AUTH_SUCCESS`,properties:this.getSIWXEventProperties()})}catch(e){let t=this.getSIWXEventProperties();(!$.state.open||R.state.view===`ApproveTransaction`)&&await $.open({view:`SIWXSignMessage`}),t.isSmartAccount?H.showError(`This application might not support Smart Accounts`):H.showError(`Signature declined`),N.sendEvent({type:`track`,event:`SIWX_AUTH_ERROR`,properties:t}),console.error(`SWIXUtil:requestSignMessage`,e)}},async cancelSignMessage(){try{this.getSIWX()?.getRequired?.()?await G.disconnect():$.close(),R.reset(`Connect`),N.sendEvent({event:`CLICK_CANCEL_SIWX`,type:`track`,properties:this.getSIWXEventProperties()})}catch(e){console.error(`SIWXUtil:cancelSignMessage`,e)}},async getSessions(){let e=M.state.siwx,t=A.getPlainAddress(J.getActiveCaipAddress()),n=J.getActiveCaipNetwork();return e&&t&&n?e.getSessions(n.caipNetworkId,t):[]},async isSIWXCloseDisabled(){let e=this.getSIWX();if(e){let t=R.state.view===`ApproveTransaction`,n=R.state.view===`SIWXSignMessage`;if(t||n)return e.getRequired?.()&&(await this.getSessions()).length===0}return!1},async universalProviderAuthenticate({universalProvider:e,chains:t,methods:n}){let r=jr.getSIWX(),i=new Set(t.map(e=>e.split(`:`)[0]));if(!r||i.size!==1||!i.has(`eip155`))return!1;let a=await r.createMessage({chainId:J.getActiveCaipNetwork()?.caipNetworkId||``,accountAddress:``}),o=await e.authenticate({nonce:a.nonce,domain:a.domain,uri:a.uri,exp:a.expirationTime,iat:a.issuedAt,nbf:a.notBefore,requestId:a.requestId,version:a.version,resources:a.resources,statement:a.statement,chainId:a.chainId,methods:n,chains:[a.chainId,...t.filter(e=>e!==a.chainId)]});if(H.showLoading(`Authenticating...`,{autoClose:!1}),Z.setConnectedWalletInfo({...o.session.peer.metadata,name:o.session.peer.metadata.name,icon:o.session.peer.metadata.icons?.[0],type:`WALLET_CONNECT`},Array.from(i)[0]),o?.auths?.length){let t=o.auths.map(t=>{let n=e.client.formatAuthMessage({request:t.p,iss:t.p.iss});return{data:{...t.p,accountAddress:t.p.iss.split(`:`).slice(-1).join(``),chainId:t.p.iss.split(`:`).slice(2,4).join(`:`),uri:t.p.aud,version:t.p.version||a.version,expirationTime:t.p.exp,issuedAt:t.p.iat,notBefore:t.p.nbf},message:n,signature:t.s.s,cacao:t}});try{await r.setSessions(t),N.sendEvent({type:`track`,event:`SIWX_AUTH_SUCCESS`,properties:jr.getSIWXEventProperties()})}catch(t){throw console.error(`SIWX:universalProviderAuth - failed to set sessions`,t),N.sendEvent({type:`track`,event:`SIWX_AUTH_ERROR`,properties:jr.getSIWXEventProperties()}),await e.disconnect().catch(console.error),t}finally{H.hide()}}return!0},getSIWXEventProperties(){return{network:J.state.activeCaipNetwork?.caipNetworkId||``,isSmartAccount:Z.state.preferredAccountType===kr.ACCOUNT_TYPES.SMART_ACCOUNT}},async clearSessions(){let e=this.getSIWX();e&&await e.setSessions([])}},U=E({transactions:[],coinbaseTransactions:{},transactionsByYear:{},lastNetworkInView:void 0,loading:!1,empty:!1,next:void 0}),Mr={state:U,subscribe(e){return D(U,()=>e(U))},setLastNetworkInView(e){U.lastNetworkInView=e},async fetchTransactions(e,t){if(!e)throw Error(`Transactions can't be fetched without an accountAddress`);U.loading=!0;try{let n=await X.fetchTransactions({account:e,cursor:U.next,onramp:t,cache:t===`coinbase`?`no-cache`:void 0,chainId:J.state.activeCaipNetwork?.caipNetworkId}),r=this.filterSpamTransactions(n.data),i=this.filterByConnectedChain(r),a=[...U.transactions,...i];U.loading=!1,t===`coinbase`?U.coinbaseTransactions=this.groupTransactionsByYearAndMonth(U.coinbaseTransactions,n.data):(U.transactions=a,U.transactionsByYear=this.groupTransactionsByYearAndMonth(U.transactionsByYear,i)),U.empty=a.length===0,U.next=n.next?n.next:void 0}catch{N.sendEvent({type:`track`,event:`ERROR_FETCH_TRANSACTIONS`,properties:{address:e,projectId:M.state.projectId,cursor:U.next,isSmartAccount:Z.state.preferredAccountType===kr.ACCOUNT_TYPES.SMART_ACCOUNT}}),H.showError(`Failed to fetch transactions`),U.loading=!1,U.empty=!0,U.next=void 0}},groupTransactionsByYearAndMonth(e={},t=[]){let n=e;return t.forEach(e=>{let t=new Date(e.metadata.minedAt).getFullYear(),r=new Date(e.metadata.minedAt).getMonth(),i=n[t]??{},a=(i[r]??[]).filter(t=>t.id!==e.id);n[t]={...i,[r]:[...a,e].sort((e,t)=>new Date(t.metadata.minedAt).getTime()-new Date(e.metadata.minedAt).getTime())}}),n},filterSpamTransactions(e){return e.filter(e=>!e.transfers.every(e=>e.nft_info?.flags.is_spam===!0))},filterByConnectedChain(e){let t=J.state.activeCaipNetwork?.caipNetworkId;return e.filter(e=>e.metadata.chain===t)},clearCursor(){U.next=void 0},resetTransactions(){U.transactions=[],U.transactionsByYear={},U.lastNetworkInView=void 0,U.loading=!1,U.empty=!1,U.next=void 0}},W=E({wcError:!1,buffering:!1,status:`disconnected`}),Nr,G={state:W,subscribeKey(e,t){return ir(W,e,t)},_getClient(){return W._client},setClient(e){W._client=rr(e)},async connectWalletConnect(){if(A.isTelegram()||A.isSafari()&&A.isIos()){if(Nr){await Nr,Nr=void 0;return}if(!A.isPairingExpired(W?.wcPairingExpiry)){W.wcUri=W.wcUri;return}Nr=this._getClient()?.connectWalletConnect?.().catch(()=>void 0),this.state.status=`connecting`,await Nr,Nr=void 0,W.wcPairingExpiry=void 0,this.state.status=`connected`}else await this._getClient()?.connectWalletConnect?.()},async connectExternal(e,t,n=!0){await this._getClient()?.connectExternal?.(e),n&&J.setActiveNamespace(t)},async reconnectExternal(e){await this._getClient()?.reconnectExternal?.(e);let t=e.chain||J.state.activeChain;t&&B.setConnectorId(e.id,t)},async setPreferredAccountType(e){$.setLoading(!0,J.state.activeChain);let t=B.getAuthConnector();t&&(await t?.provider.setPreferredAccount(e),await this.reconnectExternal(t),$.setLoading(!1,J.state.activeChain),N.sendEvent({type:`track`,event:`SET_PREFERRED_ACCOUNT_TYPE`,properties:{accountType:e,network:J.state.activeCaipNetwork?.caipNetworkId||``}}))},async signMessage(e){return this._getClient()?.signMessage(e)},parseUnits(e,t){return this._getClient()?.parseUnits(e,t)},formatUnits(e,t){return this._getClient()?.formatUnits(e,t)},async sendTransaction(e){return this._getClient()?.sendTransaction(e)},async getCapabilities(e){return this._getClient()?.getCapabilities(e)},async grantPermissions(e){return this._getClient()?.grantPermissions(e)},async walletGetAssets(e){return this._getClient()?.walletGetAssets(e)??{}},async estimateGas(e){return this._getClient()?.estimateGas(e)},async writeContract(e){return this._getClient()?.writeContract(e)},async getEnsAddress(e){return this._getClient()?.getEnsAddress(e)},async getEnsAvatar(e){return this._getClient()?.getEnsAvatar(e)},checkInstalled(e){return this._getClient()?.checkInstalled?.(e)||!1},resetWcConnection(){W.wcUri=void 0,W.wcPairingExpiry=void 0,W.wcLinking=void 0,W.recentWallet=void 0,W.status=`disconnected`,Mr.resetTransactions(),k.deleteWalletConnectDeepLink()},resetUri(){W.wcUri=void 0,W.wcPairingExpiry=void 0},finalizeWcConnection(){let{wcLinking:e,recentWallet:t}=G.state;e&&k.setWalletConnectDeepLink(e),t&&k.setAppKitRecent(t),N.sendEvent({type:`track`,event:`CONNECT_SUCCESS`,properties:{method:e?`mobile`:`qrcode`,name:R.state.data?.wallet?.name||`Unknown`}})},setWcBasic(e){W.wcBasic=e},setUri(e){W.wcUri=e,W.wcPairingExpiry=A.getPairingExpiry()},setWcLinking(e){W.wcLinking=e},setWcError(e){W.wcError=e,W.buffering=!1},setRecentWallet(e){W.recentWallet=e},setBuffering(e){W.buffering=e},setStatus(e){W.status=e},async disconnect(e){try{$.setLoading(!0,e),await jr.clearSessions(),await J.disconnect(e),$.setLoading(!1,e),B.setFilterByNamespace(void 0)}catch{throw Error(`Failed to disconnect`)}}},Pr=E({loading:!1,open:!1,selectedNetworkId:void 0,activeChain:void 0,initialized:!1}),Fr={state:Pr,subscribe(e){return D(Pr,()=>e(Pr))},set(e){Object.assign(Pr,{...Pr,...e})}};function Ir(e,t){let n=e.toString(),r=n.startsWith(`-`);r&&(n=n.slice(1)),n=n.padStart(t,`0`);let[i,a]=[n.slice(0,n.length-t),n.slice(n.length-t)];return a=a.replace(/(0+)$/,``),`${r?`-`:``}${i||`0`}${a?`.${a}`:``}`}var Lr={createBalance(e,t){let n={name:e.metadata.name||``,symbol:e.metadata.symbol||``,decimals:e.metadata.decimals||0,value:e.metadata.value||0,price:e.metadata.price||0,iconUrl:e.metadata.iconUrl||``};return{name:n.name,symbol:n.symbol,chainId:t,address:e.address===`native`?void 0:this.convertAddressToCAIP10Address(e.address,t),value:n.value,price:n.price,quantity:{decimals:n.decimals.toString(),numeric:this.convertHexToBalance({hex:e.balance,decimals:n.decimals})},iconUrl:n.iconUrl}},convertHexToBalance({hex:e,decimals:t}){return Ir(BigInt(e),t)},convertAddressToCAIP10Address(e,t){return`${t}:${e}`},createCAIP2ChainId(e,t){return`${t}:${parseInt(e,16)}`},getChainIdHexFromCAIP2ChainId(e){let t=e.split(`:`);if(t.length<2||!t[1])return`0x0`;let n=t[1],r=parseInt(n,10);return isNaN(r)?`0x0`:`0x${r.toString(16)}`},isWalletGetAssetsResponse(e){return typeof e!=`object`||!e?!1:Object.values(e).every(e=>Array.isArray(e)&&e.every(e=>this.isValidAsset(e)))},isValidAsset(e){return typeof e==`object`&&!!e&&typeof e.address==`string`&&typeof e.balance==`string`&&(e.type===`ERC20`||e.type===`NATIVE`)&&typeof e.metadata==`object`&&e.metadata!==null&&typeof e.metadata.name==`string`&&typeof e.metadata.symbol==`string`&&typeof e.metadata.decimals==`number`&&typeof e.metadata.price==`number`&&typeof e.metadata.iconUrl==`string`}},Rr={async getMyTokensWithBalance(e){let t=Z.state.address,n=J.state.activeCaipNetwork;if(!t||!n)return[];if(n.chainNamespace===`eip155`){let e=await this.getEIP155Balances(t,n);if(e)return this.filterLowQualityTokens(e)}let r=await X.getBalance(t,n.caipNetworkId,e);return this.filterLowQualityTokens(r.balances)},async getEIP155Balances(e,t){try{let n=Lr.getChainIdHexFromCAIP2ChainId(t.caipNetworkId);if(!(await G.getCapabilities(e))?.[n]?.assetDiscovery?.supported)return null;let r=await G.walletGetAssets({account:e,chainFilter:[n]});return Lr.isWalletGetAssetsResponse(r)?(r[n]||[]).map(e=>Lr.createBalance(e,t.caipNetworkId)):null}catch{return null}},filterLowQualityTokens(e){return e.filter(e=>e.quantity.decimals!==`0`)},mapBalancesToSwapTokens(e){return e?.map(e=>({...e,address:e?.address?e.address:J.getActiveNetworkTokenAddress(),decimals:parseInt(e.quantity.decimals,10),logoUri:e.iconUrl,eip2612:!1}))||[]}},K=E({tokenBalances:[],loading:!1}),zr={state:K,subscribe(e){return D(K,()=>e(K))},subscribeKey(e,t){return ir(K,e,t)},setToken(e){e&&(K.token=rr(e))},setTokenAmount(e){K.sendTokenAmount=e},setReceiverAddress(e){K.receiverAddress=e},setReceiverProfileImageUrl(e){K.receiverProfileImageUrl=e},setReceiverProfileName(e){K.receiverProfileName=e},setGasPrice(e){K.gasPrice=e},setGasPriceInUsd(e){K.gasPriceInUSD=e},setNetworkBalanceInUsd(e){K.networkBalanceInUSD=e},setLoading(e){K.loading=e},sendToken(){switch(J.state.activeCaipNetwork?.chainNamespace){case`eip155`:this.sendEvmToken();return;case`solana`:this.sendSolanaToken();return;default:throw Error(`Unsupported chain`)}},sendEvmToken(){this.state.token?.address&&this.state.sendTokenAmount&&this.state.receiverAddress?(N.sendEvent({type:`track`,event:`SEND_INITIATED`,properties:{isSmartAccount:Z.state.preferredAccountType===kr.ACCOUNT_TYPES.SMART_ACCOUNT,token:this.state.token.address,amount:this.state.sendTokenAmount,network:J.state.activeCaipNetwork?.caipNetworkId||``}}),this.sendERC20Token({receiverAddress:this.state.receiverAddress,tokenAddress:this.state.token.address,sendTokenAmount:this.state.sendTokenAmount,decimals:this.state.token.quantity.decimals})):this.state.receiverAddress&&this.state.sendTokenAmount&&this.state.gasPrice&&this.state.token?.quantity.decimals&&(N.sendEvent({type:`track`,event:`SEND_INITIATED`,properties:{isSmartAccount:Z.state.preferredAccountType===kr.ACCOUNT_TYPES.SMART_ACCOUNT,token:this.state.token?.symbol,amount:this.state.sendTokenAmount,network:J.state.activeCaipNetwork?.caipNetworkId||``}}),this.sendNativeToken({receiverAddress:this.state.receiverAddress,sendTokenAmount:this.state.sendTokenAmount,gasPrice:this.state.gasPrice,decimals:this.state.token.quantity.decimals}))},async fetchTokenBalance(e){K.loading=!0;let t=J.state.activeCaipNetwork?.caipNetworkId,n=J.state.activeCaipNetwork?.chainNamespace,r=J.state.activeCaipAddress,i=r?A.getPlainAddress(r):void 0;if(K.lastRetry&&!A.isAllowedRetry(K.lastRetry,30*O.ONE_SEC_MS))return K.loading=!1,[];try{if(i&&t&&n){let e=await Rr.getMyTokensWithBalance();return K.tokenBalances=e,K.lastRetry=void 0,e}}catch(t){K.lastRetry=Date.now(),e?.(t),H.showError(`Token Balance Unavailable`)}finally{K.loading=!1}return[]},fetchNetworkBalance(){if(K.tokenBalances.length===0)return;let e=Rr.mapBalancesToSwapTokens(K.tokenBalances);if(!e)return;let t=e.find(e=>e.address===J.getActiveNetworkTokenAddress());t&&(K.networkBalanceInUSD=t?kn.multiply(t.quantity.numeric,t.price).toString():`0`)},isInsufficientNetworkTokenForGas(e,t){let n=t||`0`;return kn.bigNumber(e).eq(0)?!0:kn.bigNumber(kn.bigNumber(n)).gt(e)},hasInsufficientGasFunds(){let e=!0;return Z.state.preferredAccountType===kr.ACCOUNT_TYPES.SMART_ACCOUNT?e=!1:K.networkBalanceInUSD&&(e=this.isInsufficientNetworkTokenForGas(K.networkBalanceInUSD,K.gasPriceInUSD)),e},async sendNativeToken(e){R.pushTransactionStack({view:`Account`,goBack:!1});let t=e.receiverAddress,n=Z.state.address,r=G.parseUnits(e.sendTokenAmount.toString(),Number(e.decimals));try{await G.sendTransaction({chainNamespace:`eip155`,to:t,address:n,data:`0x`,value:r??BigInt(0),gasPrice:e.gasPrice}),H.showSuccess(`Transaction started`),N.sendEvent({type:`track`,event:`SEND_SUCCESS`,properties:{isSmartAccount:Z.state.preferredAccountType===kr.ACCOUNT_TYPES.SMART_ACCOUNT,token:this.state.token?.symbol||``,amount:e.sendTokenAmount,network:J.state.activeCaipNetwork?.caipNetworkId||``}}),this.resetSend()}catch(t){console.error(`SendController:sendERC20Token - failed to send native token`,t);let n=t instanceof Error?t.message:`Unknown error`;N.sendEvent({type:`track`,event:`SEND_ERROR`,properties:{message:n,isSmartAccount:Z.state.preferredAccountType===kr.ACCOUNT_TYPES.SMART_ACCOUNT,token:this.state.token?.symbol||``,amount:e.sendTokenAmount,network:J.state.activeCaipNetwork?.caipNetworkId||``}}),H.showError(`Something went wrong`)}},async sendERC20Token(e){R.pushTransactionStack({view:`Account`,goBack:!1});let t=G.parseUnits(e.sendTokenAmount.toString(),Number(e.decimals));try{if(Z.state.address&&e.sendTokenAmount&&e.receiverAddress&&e.tokenAddress){let n=A.getPlainAddress(e.tokenAddress);await G.writeContract({fromAddress:Z.state.address,tokenAddress:n,args:[e.receiverAddress,t??BigInt(0)],method:`transfer`,abi:Nn.getERC20Abi(n),chainNamespace:`eip155`}),H.showSuccess(`Transaction started`),this.resetSend()}}catch(t){console.error(`SendController:sendERC20Token - failed to send erc20 token`,t);let n=t instanceof Error?t.message:`Unknown error`;N.sendEvent({type:`track`,event:`SEND_ERROR`,properties:{message:n,isSmartAccount:Z.state.preferredAccountType===kr.ACCOUNT_TYPES.SMART_ACCOUNT,token:this.state.token?.symbol||``,amount:e.sendTokenAmount,network:J.state.activeCaipNetwork?.caipNetworkId||``}}),H.showError(`Something went wrong`)}},sendSolanaToken(){if(!this.state.sendTokenAmount||!this.state.receiverAddress){H.showError(`Please enter a valid amount and receiver address`);return}R.pushTransactionStack({view:`Account`,goBack:!1}),G.sendTransaction({chainNamespace:`solana`,to:this.state.receiverAddress,value:this.state.sendTokenAmount}).then(()=>{this.resetSend(),Z.fetchTokenBalance()}).catch(e=>{H.showError(`Failed to send transaction. Please try again.`),console.error(`SendController:sendToken - failed to send solana transaction`,e)})},resetSend(){K.token=void 0,K.sendTokenAmount=void 0,K.receiverAddress=void 0,K.receiverProfileImageUrl=void 0,K.receiverProfileName=void 0,K.loading=!1,K.tokenBalances=[]}},Br={currentTab:0,tokenBalance:[],smartAccountDeployed:!1,addressLabels:new Map,allAccounts:[],user:void 0},Vr={caipNetwork:void 0,supportsAllNetworks:!0,smartAccountEnabledNetworks:[]},q=E({chains:ar(),activeCaipAddress:void 0,activeChain:void 0,activeCaipNetwork:void 0,noAdapters:!1,universalAdapter:{networkControllerClient:void 0,connectionControllerClient:void 0},isSwitchingNamespace:!1}),J={state:q,subscribe(e){return D(q,()=>{e(q)})},subscribeKey(e,t){return ir(q,e,t)},subscribeChainProp(e,t,n){let r;return D(q.chains,()=>{let i=n||q.activeChain;if(i){let n=q.chains.get(i)?.[e];r!==n&&(r=n,t(n))}})},initialize(e,t,n){let{chainId:r,namespace:i}=k.getActiveNetworkProps(),a=t?.find(e=>e.id.toString()===r?.toString()),o=e.find(e=>e?.namespace===i)||e?.[0],s=new Set([...t?.map(e=>e.chainNamespace)??[]]);(e?.length===0||!o)&&(q.noAdapters=!0),q.noAdapters||(q.activeChain=o?.namespace,q.activeCaipNetwork=a,this.setChainNetworkData(o?.namespace,{caipNetwork:a}),q.activeChain&&Fr.set({activeChain:o?.namespace})),s.forEach(e=>{let r=t?.filter(t=>t.chainNamespace===e);J.state.chains.set(e,{namespace:e,networkState:E({...Vr,caipNetwork:r?.[0]}),accountState:E(Br),caipNetworks:r??[],...n}),this.setRequestedCaipNetworks(r??[],e)})},removeAdapter(e){if(q.activeChain===e){let t=Array.from(q.chains.entries()).find(([t])=>t!==e);if(t){let e=t[1]?.caipNetworks?.[0];e&&this.setActiveCaipNetwork(e)}}q.chains.delete(e)},addAdapter(e,{networkControllerClient:t,connectionControllerClient:n},r){q.chains.set(e.namespace,{namespace:e.namespace,networkState:{...Vr,caipNetwork:r[0]},accountState:Br,caipNetworks:r,connectionControllerClient:n,networkControllerClient:t}),this.setRequestedCaipNetworks(r?.filter(t=>t.chainNamespace===e.namespace)??[],e.namespace)},addNetwork(e){let t=q.chains.get(e.chainNamespace);if(t){let n=[...t.caipNetworks||[]];t.caipNetworks?.find(t=>t.id===e.id)||n.push(e),q.chains.set(e.chainNamespace,{...t,caipNetworks:n}),this.setRequestedCaipNetworks(n,e.chainNamespace)}},removeNetwork(e,t){let n=q.chains.get(e);if(n){let r=q.activeCaipNetwork?.id===t,i=[...n.caipNetworks?.filter(e=>e.id!==t)||[]];r&&n?.caipNetworks?.[0]&&this.setActiveCaipNetwork(n.caipNetworks[0]),q.chains.set(e,{...n,caipNetworks:i}),this.setRequestedCaipNetworks(i||[],e)}},setAdapterNetworkState(e,t){let n=q.chains.get(e);n&&(n.networkState={...n.networkState||Vr,...t},q.chains.set(e,n))},setChainAccountData(e,t,n=!0){if(!e)throw Error(`Chain is required to update chain account data`);let r=q.chains.get(e);if(r){let n={...r.accountState||Br,...t};q.chains.set(e,{...r,accountState:n}),(q.chains.size===1||q.activeChain===e)&&(t.caipAddress&&(q.activeCaipAddress=t.caipAddress),Z.replaceState(n))}},setChainNetworkData(e,t){if(!e)return;let n=q.chains.get(e);if(n){let r={...n.networkState||Vr,...t};q.chains.set(e,{...n,networkState:r})}},setAccountProp(e,t,n,r=!0){this.setChainAccountData(n,{[e]:t},r),e===`status`&&t===`disconnected`&&n&&B.removeConnectorId(n)},setActiveNamespace(e){q.activeChain=e;let t=e?q.chains.get(e):void 0,n=t?.networkState?.caipNetwork;n?.id&&e&&(q.activeCaipAddress=t?.accountState?.caipAddress,q.activeCaipNetwork=n,this.setChainNetworkData(e,{caipNetwork:n}),k.setActiveCaipNetworkId(n?.caipNetworkId),Fr.set({activeChain:e,selectedNetworkId:n?.caipNetworkId}))},setActiveCaipNetwork(e){if(!e)return;q.activeChain!==e.chainNamespace&&this.setIsSwitchingNamespace(!0);let t=q.chains.get(e.chainNamespace);q.activeChain=e.chainNamespace,q.activeCaipNetwork=e,this.setChainNetworkData(e.chainNamespace,{caipNetwork:e}),t?.accountState?.address?q.activeCaipAddress=`${e.chainNamespace}:${e.id}:${t?.accountState?.address}`:q.activeCaipAddress=void 0,this.setAccountProp(`caipAddress`,q.activeCaipAddress,e.chainNamespace),t&&Z.replaceState(t.accountState),zr.resetSend(),Fr.set({activeChain:q.activeChain,selectedNetworkId:q.activeCaipNetwork?.caipNetworkId}),k.setActiveCaipNetworkId(e.caipNetworkId),!this.checkIfSupportedNetwork(e.chainNamespace)&&M.state.enableNetworkSwitch&&!M.state.allowUnsupportedChain&&!G.state.wcBasic&&this.showUnsupportedChainUI()},addCaipNetwork(e){if(!e)return;let t=q.chains.get(e.chainNamespace);t&&t?.caipNetworks?.push(e)},async switchActiveNamespace(e){if(!e)return;let t=e!==J.state.activeChain,n=J.getNetworkData(e)?.caipNetwork,r=J.getCaipNetworkByNamespace(e,n?.id);t&&r&&await J.switchActiveNetwork(r)},async switchActiveNetwork(e){J.state.chains.get(J.state.activeChain)?.caipNetworks?.some(e=>e.id===q.activeCaipNetwork?.id)||R.goBack();let t=this.getNetworkControllerClient(e.chainNamespace);t&&(await t.switchCaipNetwork(e),N.sendEvent({type:`track`,event:`SWITCH_NETWORK`,properties:{network:e.caipNetworkId}}))},getNetworkControllerClient(e){let t=e||q.activeChain,n=q.chains.get(t);if(!n)throw Error(`Chain adapter not found`);if(!n.networkControllerClient)throw Error(`NetworkController client not set`);return n.networkControllerClient},getConnectionControllerClient(e){let t=e||q.activeChain;if(!t)throw Error(`Chain is required to get connection controller client`);let n=q.chains.get(t);if(!n?.connectionControllerClient)throw Error(`ConnectionController client not set`);return n.connectionControllerClient},getAccountProp(e,t){let n=q.activeChain;if(t&&(n=t),!n)return;let r=q.chains.get(n)?.accountState;if(r)return r[e]},getNetworkProp(e,t){let n=q.chains.get(t)?.networkState;if(n)return n[e]},getRequestedCaipNetworks(e){let{approvedCaipNetworkIds:t=[],requestedCaipNetworks:n=[]}=q.chains.get(e)?.networkState||{};return A.sortRequestedNetworks(t,n)},getAllRequestedCaipNetworks(){let e=[];return q.chains.forEach(t=>{let n=this.getRequestedCaipNetworks(t.namespace);e.push(...n)}),e},setRequestedCaipNetworks(e,t){this.setAdapterNetworkState(t,{requestedCaipNetworks:e})},getAllApprovedCaipNetworkIds(){let e=[];return q.chains.forEach(t=>{let n=this.getApprovedCaipNetworkIds(t.namespace);e.push(...n)}),e},getActiveCaipNetwork(){return q.activeCaipNetwork},getActiveCaipAddress(){return q.activeCaipAddress},getApprovedCaipNetworkIds(e){return q.chains.get(e)?.networkState?.approvedCaipNetworkIds||[]},async setApprovedCaipNetworksData(e){let t=await this.getNetworkControllerClient()?.getApprovedCaipNetworksData();this.setAdapterNetworkState(e,{approvedCaipNetworkIds:t?.approvedCaipNetworkIds,supportsAllNetworks:t?.supportsAllNetworks})},checkIfSupportedNetwork(e,t){let n=t||q.activeCaipNetwork,r=this.getRequestedCaipNetworks(e);return r.length?r?.some(e=>e.id===n?.id):!0},checkIfSupportedChainId(e){return q.activeChain?this.getRequestedCaipNetworks(q.activeChain)?.some(t=>t.id===e):!0},setSmartAccountEnabledNetworks(e,t){this.setAdapterNetworkState(t,{smartAccountEnabledNetworks:e})},checkIfSmartAccountEnabled(){let e=ln.caipNetworkIdToNumber(q.activeCaipNetwork?.caipNetworkId),t=q.activeChain;return!t||!e?!1:!!this.getNetworkProp(`smartAccountEnabledNetworks`,t)?.includes(Number(e))},getActiveNetworkTokenAddress(){let e=q.activeCaipNetwork?.chainNamespace||`eip155`;return`${e}:${q.activeCaipNetwork?.id||1}:${O.NATIVE_TOKEN_ADDRESS[e]}`},showUnsupportedChainUI(){$.open({view:`UnsupportedChain`})},checkIfNamesSupported(){let e=q.activeCaipNetwork;return!!(e?.chainNamespace&&O.NAMES_SUPPORTED_CHAIN_NAMESPACES.includes(e.chainNamespace))},resetNetwork(e){this.setAdapterNetworkState(e,{approvedCaipNetworkIds:void 0,supportsAllNetworks:!0,smartAccountEnabledNetworks:[]})},resetAccount(e){let t=e;if(!t)throw Error(`Chain is required to set account prop`);q.activeCaipAddress=void 0,this.setChainAccountData(t,{smartAccountDeployed:!1,currentTab:0,caipAddress:void 0,address:void 0,balance:void 0,balanceSymbol:void 0,profileName:void 0,profileImage:void 0,addressExplorerUrl:void 0,tokenBalance:[],connectedWalletInfo:void 0,preferredAccountType:void 0,socialProvider:void 0,socialWindow:void 0,farcasterUrl:void 0,allAccounts:[],user:void 0,status:`disconnected`}),B.removeConnectorId(t)},async disconnect(e){let t=Or(e);try{zr.resetSend();let n=await Promise.allSettled(t.map(async([e,t])=>{try{let{caipAddress:n}=this.getAccountData(e)||{};n&&t.connectionControllerClient?.disconnect&&await t.connectionControllerClient.disconnect(e),this.resetAccount(e),this.resetNetwork(e)}catch(t){throw Error(`Failed to disconnect chain ${e}: ${t.message}`)}}));G.resetWcConnection();let r=n.filter(e=>e.status===`rejected`);if(r.length>0)throw Error(r.map(e=>e.reason.message).join(`, `));k.deleteConnectedSocialProvider(),e?B.removeConnectorId(e):B.resetConnectorIds(),N.sendEvent({type:`track`,event:`DISCONNECT_SUCCESS`,properties:{namespace:e||`all`}})}catch(e){console.error(e.message||`Failed to disconnect chains`),N.sendEvent({type:`track`,event:`DISCONNECT_ERROR`,properties:{message:e.message||`Failed to disconnect chains`}})}},setIsSwitchingNamespace(e){q.isSwitchingNamespace=e},getFirstCaipNetworkSupportsAuthConnector(){let e=[],t;if(q.chains.forEach(t=>{C.AUTH_CONNECTOR_SUPPORTED_CHAINS.find(e=>e===t.namespace)&&t.namespace&&e.push(t.namespace)}),e.length>0){let n=e[0];return t=n?q.chains.get(n)?.caipNetworks?.[0]:void 0,t}},getAccountData(e){return e?J.state.chains.get(e)?.accountState:Z.state},getNetworkData(e){let t=e||q.activeChain;if(t)return J.state.chains.get(t)?.networkState},getCaipNetworkByNamespace(e,t){if(!e)return;let n=J.state.chains.get(e);return n?.caipNetworks?.find(e=>e.id===t)||n?.networkState?.caipNetwork||n?.caipNetworks?.[0]},getRequestedCaipNetworkIds(){let e=B.state.filterByNamespace;return(e?[q.chains.get(e)]:Array.from(q.chains.values())).flatMap(e=>e?.caipNetworks||[]).map(e=>e.caipNetworkId)}},Hr={purchaseCurrencies:[{id:`2b92315d-eab7-5bef-84fa-089a131333f5`,name:`USD Coin`,symbol:`USDC`,networks:[{name:`ethereum-mainnet`,display_name:`Ethereum`,chain_id:`1`,contract_address:`0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`},{name:`polygon-mainnet`,display_name:`Polygon`,chain_id:`137`,contract_address:`0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`}]},{id:`2b92315d-eab7-5bef-84fa-089a131333f5`,name:`Ether`,symbol:`ETH`,networks:[{name:`ethereum-mainnet`,display_name:`Ethereum`,chain_id:`1`,contract_address:`0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`},{name:`polygon-mainnet`,display_name:`Polygon`,chain_id:`137`,contract_address:`0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`}]}],paymentCurrencies:[{id:`USD`,payment_method_limits:[{id:`card`,min:`10.00`,max:`7500.00`},{id:`ach_bank_account`,min:`10.00`,max:`25000.00`}]},{id:`EUR`,payment_method_limits:[{id:`card`,min:`10.00`,max:`7500.00`},{id:`ach_bank_account`,min:`10.00`,max:`25000.00`}]}]},Ur=A.getBlockchainApiUrl(),Y=E({clientId:null,api:new cr({baseUrl:Ur,clientId:null}),supportedChains:{http:[],ws:[]}}),X={state:Y,async get(e){let{st:t,sv:n}=X.getSdkProperties(),r=M.state.projectId,i={...e.params||{},st:t,sv:n,projectId:r};return Y.api.get({...e,params:i})},getSdkProperties(){let{sdkType:e,sdkVersion:t}=M.state;return{st:e||`unknown`,sv:t||`unknown`}},async isNetworkSupported(e){if(!e)return!1;try{Y.supportedChains.http.length||await X.getSupportedNetworks()}catch{return!1}return Y.supportedChains.http.includes(e)},async getSupportedNetworks(){let e=await X.get({path:`v1/supported-chains`});return Y.supportedChains=e,e},async fetchIdentity({address:e,caipNetworkId:t}){if(!await X.isNetworkSupported(t))return{avatar:``,name:``};let n=k.getIdentityFromCacheForAddress(e);if(n)return n;let r=await X.get({path:`/v1/identity/${e}`,params:{sender:J.state.activeCaipAddress?A.getPlainAddress(J.state.activeCaipAddress):void 0}});return k.updateIdentityCache({address:e,identity:r,timestamp:Date.now()}),r},async fetchTransactions({account:e,cursor:t,onramp:n,signal:r,cache:i,chainId:a}){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?X.get({path:`/v1/account/${e}/history`,params:{cursor:t,onramp:n,chainId:a},signal:r,cache:i}):{data:[],next:void 0}},async fetchSwapQuote({amount:e,userAddress:t,from:n,to:r,gasPrice:i}){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?X.get({path:`/v1/convert/quotes`,headers:{"Content-Type":`application/json`},params:{amount:e,userAddress:t,from:n,to:r,gasPrice:i}}):{quotes:[]}},async fetchSwapTokens({chainId:e}){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?X.get({path:`/v1/convert/tokens`,params:{chainId:e}}):{tokens:[]}},async fetchTokenPrice({addresses:e}){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?Y.api.post({path:`/v1/fungible/price`,body:{currency:`usd`,addresses:e,projectId:M.state.projectId},headers:{"Content-Type":`application/json`}}):{fungibles:[]}},async fetchSwapAllowance({tokenAddress:e,userAddress:t}){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?X.get({path:`/v1/convert/allowance`,params:{tokenAddress:e,userAddress:t},headers:{"Content-Type":`application/json`}}):{allowance:`0`}},async fetchGasPrice({chainId:e}){let{st:t,sv:n}=X.getSdkProperties();if(!await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId))throw Error(`Network not supported for Gas Price`);return X.get({path:`/v1/convert/gas-price`,headers:{"Content-Type":`application/json`},params:{chainId:e,st:t,sv:n}})},async generateSwapCalldata({amount:e,from:t,to:n,userAddress:r,disableEstimate:i}){if(!await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId))throw Error(`Network not supported for Swaps`);return Y.api.post({path:`/v1/convert/build-transaction`,headers:{"Content-Type":`application/json`},body:{amount:e,eip155:{slippage:O.CONVERT_SLIPPAGE_TOLERANCE},projectId:M.state.projectId,from:t,to:n,userAddress:r,disableEstimate:i}})},async generateApproveCalldata({from:e,to:t,userAddress:n}){let{st:r,sv:i}=X.getSdkProperties();if(!await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId))throw Error(`Network not supported for Swaps`);return X.get({path:`/v1/convert/build-approve`,headers:{"Content-Type":`application/json`},params:{userAddress:n,from:e,to:t,st:r,sv:i}})},async getBalance(e,t,n){let{st:r,sv:i}=X.getSdkProperties();if(!await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId))return H.showError(`Token Balance Unavailable`),{balances:[]};let a=`${t}:${e}`,o=k.getBalanceCacheForCaipAddress(a);if(o)return o;let s=await X.get({path:`/v1/account/${e}/balance`,params:{currency:`usd`,chainId:t,forceUpdate:n,st:r,sv:i}});return k.updateBalanceCache({caipAddress:a,balance:s,timestamp:Date.now()}),s},async lookupEnsName(e){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?X.get({path:`/v1/profile/account/${e}`,params:{apiVersion:`2`}}):{addresses:{},attributes:[]}},async reverseLookupEnsName({address:e}){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?X.get({path:`/v1/profile/reverse/${e}`,params:{sender:Z.state.address,apiVersion:`2`}}):[]},async getEnsNameSuggestions(e){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?X.get({path:`/v1/profile/suggestions/${e}`,params:{zone:`reown.id`}}):{suggestions:[]}},async registerEnsName({coinType:e,address:t,message:n,signature:r}){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?Y.api.post({path:`/v1/profile/account`,body:{coin_type:e,address:t,message:n,signature:r},headers:{"Content-Type":`application/json`}}):{success:!1}},async generateOnRampURL({destinationWallets:e,partnerUserId:t,defaultNetwork:n,purchaseAmount:r,paymentAmount:i}){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?(await Y.api.post({path:`/v1/generators/onrampurl`,params:{projectId:M.state.projectId},body:{destinationWallets:e,defaultNetwork:n,partnerUserId:t,defaultExperience:`buy`,presetCryptoAmount:r,presetFiatAmount:i}})).url:``},async getOnrampOptions(){if(!await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId))return{paymentCurrencies:[],purchaseCurrencies:[]};try{return await X.get({path:`/v1/onramp/options`})}catch{return Hr}},async getOnrampQuote({purchaseCurrency:e,paymentCurrency:t,amount:n,network:r}){try{return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?await Y.api.post({path:`/v1/onramp/quote`,params:{projectId:M.state.projectId},body:{purchaseCurrency:e,paymentCurrency:t,amount:n,network:r}}):null}catch{return{coinbaseFee:{amount:n,currency:t.id},networkFee:{amount:n,currency:t.id},paymentSubtotal:{amount:n,currency:t.id},paymentTotal:{amount:n,currency:t.id},purchaseAmount:{amount:n,currency:t.id},quoteId:`mocked-quote-id`}}},async getSmartSessions(e){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?X.get({path:`/v1/sessions/${e}`}):[]},async revokeSmartSession(e,t,n){return await X.isNetworkSupported(J.state.activeCaipNetwork?.caipNetworkId)?Y.api.post({path:`/v1/sessions/${e}/revoke`,params:{projectId:M.state.projectId},body:{pci:t,signature:n}}):{success:!1}},setClientId(e){Y.clientId=e,Y.api=new cr({baseUrl:Ur,clientId:e})}},Wr=E({currentTab:0,tokenBalance:[],smartAccountDeployed:!1,addressLabels:new Map,allAccounts:[]}),Z={state:Wr,replaceState(e){e&&Object.assign(Wr,rr(e))},subscribe(e){return J.subscribeChainProp(`accountState`,t=>{if(t)return e(t)})},subscribeKey(e,t,n){let r;return J.subscribeChainProp(`accountState`,n=>{if(n){let i=n[e];r!==i&&(r=i,t(i))}},n)},setStatus(e,t){J.setAccountProp(`status`,e,t)},getCaipAddress(e){return J.getAccountProp(`caipAddress`,e)},setCaipAddress(e,t){let n=e?A.getPlainAddress(e):void 0;t===J.state.activeChain&&(J.state.activeCaipAddress=e),J.setAccountProp(`caipAddress`,e,t),J.setAccountProp(`address`,n,t)},setBalance(e,t,n){J.setAccountProp(`balance`,e,n),J.setAccountProp(`balanceSymbol`,t,n)},setProfileName(e,t){J.setAccountProp(`profileName`,e,t)},setProfileImage(e,t){J.setAccountProp(`profileImage`,e,t)},setUser(e,t){J.setAccountProp(`user`,e,t)},setAddressExplorerUrl(e,t){J.setAccountProp(`addressExplorerUrl`,e,t)},setSmartAccountDeployed(e,t){J.setAccountProp(`smartAccountDeployed`,e,t)},setCurrentTab(e){J.setAccountProp(`currentTab`,e,J.state.activeChain)},setTokenBalance(e,t){e&&J.setAccountProp(`tokenBalance`,e,t)},setShouldUpdateToAddress(e,t){J.setAccountProp(`shouldUpdateToAddress`,e,t)},setAllAccounts(e,t){J.setAccountProp(`allAccounts`,e,t)},addAddressLabel(e,t,n){let r=J.getAccountProp(`addressLabels`,n)||new Map;r.set(e,t),J.setAccountProp(`addressLabels`,r,n)},removeAddressLabel(e,t){let n=J.getAccountProp(`addressLabels`,t)||new Map;n.delete(e),J.setAccountProp(`addressLabels`,n,t)},setConnectedWalletInfo(e,t){J.setAccountProp(`connectedWalletInfo`,e,t,!1)},setPreferredAccountType(e,t){J.setAccountProp(`preferredAccountType`,e,t)},setSocialProvider(e,t){e&&J.setAccountProp(`socialProvider`,e,t)},setSocialWindow(e,t){J.setAccountProp(`socialWindow`,e?rr(e):void 0,t)},setFarcasterUrl(e,t){J.setAccountProp(`farcasterUrl`,e,t)},async fetchTokenBalance(e){Wr.balanceLoading=!0;let t=J.state.activeCaipNetwork?.caipNetworkId,n=J.state.activeCaipNetwork?.chainNamespace,r=J.state.activeCaipAddress,i=r?A.getPlainAddress(r):void 0;if(Wr.lastRetry&&!A.isAllowedRetry(Wr.lastRetry,30*O.ONE_SEC_MS))return Wr.balanceLoading=!1,[];try{if(i&&t&&n){let e=(await X.getBalance(i,t)).balances.filter(e=>e.quantity.decimals!==`0`);return this.setTokenBalance(e,n),Wr.lastRetry=void 0,Wr.balanceLoading=!1,e}}catch(t){Wr.lastRetry=Date.now(),e?.(t),H.showError(`Token Balance Unavailable`)}finally{Wr.balanceLoading=!1}return[]},resetAccount(e){J.resetAccount(e)}},Q=E({loading:!1,loadingNamespaceMap:new Map,open:!1,shake:!1,namespace:void 0}),$={state:Q,subscribe(e){return D(Q,()=>e(Q))},subscribeKey(e,t){return ir(Q,e,t)},async open(e){let t=Z.state.status===`connected`;G.state.wcBasic?I.prefetch({fetchNetworkImages:!1,fetchConnectorImages:!1}):await I.prefetch({fetchConnectorImages:!t,fetchFeaturedWallets:!t,fetchRecommendedWallets:!t}),e?.namespace?(await J.switchActiveNamespace(e.namespace),$.setLoading(!0,e.namespace)):$.setLoading(!0),B.setFilterByNamespace(e?.namespace);let n=J.getAccountData(e?.namespace)?.caipAddress;J.state.noAdapters&&!n?A.isMobile()?R.reset(`AllWallets`):R.reset(`ConnectingWalletConnectBasic`):e?.view?R.reset(e.view):n?R.reset(`Account`):R.reset(`Connect`),Q.open=!0,Fr.set({open:!0}),N.sendEvent({type:`track`,event:`MODAL_OPEN`,properties:{connected:!!n}})},close(){let e=M.state.enableEmbedded,t=!!J.state.activeCaipAddress;Q.open&&N.sendEvent({type:`track`,event:`MODAL_CLOSE`,properties:{connected:t}}),Q.open=!1,$.clearLoading(),e?t?R.replace(`Account`):R.push(`Connect`):Fr.set({open:!1}),G.resetUri()},setLoading(e,t){t&&Q.loadingNamespaceMap.set(t,e),Q.loading=e,Fr.set({loading:e})},clearLoading(){Q.loadingNamespaceMap.clear(),Q.loading=!1},shake(){Q.shake||(Q.shake=!0,setTimeout(()=>{Q.shake=!1},500))}},Gr={ACCOUNT_TABS:[{label:`Tokens`},{label:`NFTs`},{label:`Activity`}],SECURE_SITE_ORIGIN:(typeof process<`u`?{}.NEXT_PUBLIC_SECURE_SITE_ORIGIN:void 0)||`https://secure.walletconnect.org`,VIEW_DIRECTION:{Next:`next`,Prev:`prev`},DEFAULT_CONNECT_METHOD_ORDER:[`email`,`social`,`wallet`],ANIMATION_DURATIONS:{HeaderText:120,ModalHeight:150,ViewTransition:150}},Kr=void 0,qr=void 0,Jr=void 0;function Yr(e,t){Kr=document.createElement(`style`),qr=document.createElement(`style`),Jr=document.createElement(`style`),Kr.textContent=Qr(e).core.cssText,qr.textContent=Qr(e).dark.cssText,Jr.textContent=Qr(e).light.cssText,document.head.appendChild(Kr),document.head.appendChild(qr),document.head.appendChild(Jr),Xr(t)}function Xr(e){qr&&Jr&&(e===`light`?(qr.removeAttribute(`media`),Jr.media=`enabled`):(Jr.removeAttribute(`media`),qr.media=`enabled`))}function Zr(e){Kr&&qr&&Jr&&(Kr.textContent=Qr(e).core.cssText,qr.textContent=Qr(e).dark.cssText,Jr.textContent=Qr(e).light.cssText)}function Qr(e){return{core:o`
      @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');
      @keyframes w3m-shake {
        0% {
          transform: scale(1) rotate(0deg);
        }
        20% {
          transform: scale(1) rotate(-1deg);
        }
        40% {
          transform: scale(1) rotate(1.5deg);
        }
        60% {
          transform: scale(1) rotate(-1.5deg);
        }
        80% {
          transform: scale(1) rotate(1deg);
        }
        100% {
          transform: scale(1) rotate(0deg);
        }
      }
      @keyframes w3m-iframe-fade-out {
        0% {
          opacity: 1;
        }
        100% {
          opacity: 0;
        }
      }
      @keyframes w3m-iframe-zoom-in {
        0% {
          transform: translateY(50px);
          opacity: 0;
        }
        100% {
          transform: translateY(0px);
          opacity: 1;
        }
      }
      @keyframes w3m-iframe-zoom-in-mobile {
        0% {
          transform: scale(0.95);
          opacity: 0;
        }
        100% {
          transform: scale(1);
          opacity: 1;
        }
      }
      :root {
        --w3m-modal-width: 360px;
        --w3m-color-mix-strength: ${s(e?.[`--w3m-color-mix-strength`]?`${e[`--w3m-color-mix-strength`]}%`:`0%`)};
        --w3m-font-family: ${s(e?.[`--w3m-font-family`]||`Inter, Segoe UI, Roboto, Oxygen, Ubuntu, Cantarell, Fira Sans, Droid Sans, Helvetica Neue, sans-serif;`)};
        --w3m-font-size-master: ${s(e?.[`--w3m-font-size-master`]||`10px`)};
        --w3m-border-radius-master: ${s(e?.[`--w3m-border-radius-master`]||`4px`)};
        --w3m-z-index: ${s(e?.[`--w3m-z-index`]||999)};

        --wui-font-family: var(--w3m-font-family);

        --wui-font-size-mini: calc(var(--w3m-font-size-master) * 0.8);
        --wui-font-size-micro: var(--w3m-font-size-master);
        --wui-font-size-tiny: calc(var(--w3m-font-size-master) * 1.2);
        --wui-font-size-small: calc(var(--w3m-font-size-master) * 1.4);
        --wui-font-size-paragraph: calc(var(--w3m-font-size-master) * 1.6);
        --wui-font-size-medium: calc(var(--w3m-font-size-master) * 1.8);
        --wui-font-size-large: calc(var(--w3m-font-size-master) * 2);
        --wui-font-size-title-6: calc(var(--w3m-font-size-master) * 2.2);
        --wui-font-size-medium-title: calc(var(--w3m-font-size-master) * 2.4);
        --wui-font-size-2xl: calc(var(--w3m-font-size-master) * 4);

        --wui-border-radius-5xs: var(--w3m-border-radius-master);
        --wui-border-radius-4xs: calc(var(--w3m-border-radius-master) * 1.5);
        --wui-border-radius-3xs: calc(var(--w3m-border-radius-master) * 2);
        --wui-border-radius-xxs: calc(var(--w3m-border-radius-master) * 3);
        --wui-border-radius-xs: calc(var(--w3m-border-radius-master) * 4);
        --wui-border-radius-s: calc(var(--w3m-border-radius-master) * 5);
        --wui-border-radius-m: calc(var(--w3m-border-radius-master) * 7);
        --wui-border-radius-l: calc(var(--w3m-border-radius-master) * 9);
        --wui-border-radius-3xl: calc(var(--w3m-border-radius-master) * 20);

        --wui-font-weight-light: 400;
        --wui-font-weight-regular: 500;
        --wui-font-weight-medium: 600;
        --wui-font-weight-bold: 700;

        --wui-letter-spacing-2xl: -1.6px;
        --wui-letter-spacing-medium-title: -0.96px;
        --wui-letter-spacing-title-6: -0.88px;
        --wui-letter-spacing-large: -0.8px;
        --wui-letter-spacing-medium: -0.72px;
        --wui-letter-spacing-paragraph: -0.64px;
        --wui-letter-spacing-small: -0.56px;
        --wui-letter-spacing-tiny: -0.48px;
        --wui-letter-spacing-micro: -0.2px;
        --wui-letter-spacing-mini: -0.16px;

        --wui-spacing-0: 0px;
        --wui-spacing-4xs: 2px;
        --wui-spacing-3xs: 4px;
        --wui-spacing-xxs: 6px;
        --wui-spacing-2xs: 7px;
        --wui-spacing-xs: 8px;
        --wui-spacing-1xs: 10px;
        --wui-spacing-s: 12px;
        --wui-spacing-m: 14px;
        --wui-spacing-l: 16px;
        --wui-spacing-2l: 18px;
        --wui-spacing-xl: 20px;
        --wui-spacing-xxl: 24px;
        --wui-spacing-2xl: 32px;
        --wui-spacing-3xl: 40px;
        --wui-spacing-4xl: 90px;
        --wui-spacing-5xl: 95px;

        --wui-icon-box-size-xxs: 14px;
        --wui-icon-box-size-xs: 20px;
        --wui-icon-box-size-sm: 24px;
        --wui-icon-box-size-md: 32px;
        --wui-icon-box-size-mdl: 36px;
        --wui-icon-box-size-lg: 40px;
        --wui-icon-box-size-2lg: 48px;
        --wui-icon-box-size-xl: 64px;

        --wui-icon-size-inherit: inherit;
        --wui-icon-size-xxs: 10px;
        --wui-icon-size-xs: 12px;
        --wui-icon-size-sm: 14px;
        --wui-icon-size-md: 16px;
        --wui-icon-size-mdl: 18px;
        --wui-icon-size-lg: 20px;
        --wui-icon-size-xl: 24px;
        --wui-icon-size-xxl: 28px;

        --wui-wallet-image-size-inherit: inherit;
        --wui-wallet-image-size-sm: 40px;
        --wui-wallet-image-size-md: 56px;
        --wui-wallet-image-size-lg: 80px;

        --wui-visual-size-size-inherit: inherit;
        --wui-visual-size-sm: 40px;
        --wui-visual-size-md: 55px;
        --wui-visual-size-lg: 80px;

        --wui-box-size-md: 100px;
        --wui-box-size-lg: 120px;

        --wui-ease-out-power-2: cubic-bezier(0, 0, 0.22, 1);
        --wui-ease-out-power-1: cubic-bezier(0, 0, 0.55, 1);

        --wui-ease-in-power-3: cubic-bezier(0.66, 0, 1, 1);
        --wui-ease-in-power-2: cubic-bezier(0.45, 0, 1, 1);
        --wui-ease-in-power-1: cubic-bezier(0.3, 0, 1, 1);

        --wui-ease-inout-power-1: cubic-bezier(0.45, 0, 0.55, 1);

        --wui-duration-lg: 200ms;
        --wui-duration-md: 125ms;
        --wui-duration-sm: 75ms;

        --wui-path-network-sm: path(
          'M15.4 2.1a5.21 5.21 0 0 1 5.2 0l11.61 6.7a5.21 5.21 0 0 1 2.61 4.52v13.4c0 1.87-1 3.59-2.6 4.52l-11.61 6.7c-1.62.93-3.6.93-5.22 0l-11.6-6.7a5.21 5.21 0 0 1-2.61-4.51v-13.4c0-1.87 1-3.6 2.6-4.52L15.4 2.1Z'
        );

        --wui-path-network-md: path(
          'M43.4605 10.7248L28.0485 1.61089C25.5438 0.129705 22.4562 0.129705 19.9515 1.61088L4.53951 10.7248C2.03626 12.2051 0.5 14.9365 0.5 17.886V36.1139C0.5 39.0635 2.03626 41.7949 4.53951 43.2752L19.9515 52.3891C22.4562 53.8703 25.5438 53.8703 28.0485 52.3891L43.4605 43.2752C45.9637 41.7949 47.5 39.0635 47.5 36.114V17.8861C47.5 14.9365 45.9637 12.2051 43.4605 10.7248Z'
        );

        --wui-path-network-lg: path(
          'M78.3244 18.926L50.1808 2.45078C45.7376 -0.150261 40.2624 -0.150262 35.8192 2.45078L7.6756 18.926C3.23322 21.5266 0.5 26.3301 0.5 31.5248V64.4752C0.5 69.6699 3.23322 74.4734 7.6756 77.074L35.8192 93.5492C40.2624 96.1503 45.7376 96.1503 50.1808 93.5492L78.3244 77.074C82.7668 74.4734 85.5 69.6699 85.5 64.4752V31.5248C85.5 26.3301 82.7668 21.5266 78.3244 18.926Z'
        );

        --wui-width-network-sm: 36px;
        --wui-width-network-md: 48px;
        --wui-width-network-lg: 86px;

        --wui-height-network-sm: 40px;
        --wui-height-network-md: 54px;
        --wui-height-network-lg: 96px;

        --wui-icon-size-network-xs: 12px;
        --wui-icon-size-network-sm: 16px;
        --wui-icon-size-network-md: 24px;
        --wui-icon-size-network-lg: 42px;

        --wui-color-inherit: inherit;

        --wui-color-inverse-100: #fff;
        --wui-color-inverse-000: #000;

        --wui-cover: rgba(20, 20, 20, 0.8);

        --wui-color-modal-bg: var(--wui-color-modal-bg-base);

        --wui-color-accent-100: var(--wui-color-accent-base-100);
        --wui-color-accent-090: var(--wui-color-accent-base-090);
        --wui-color-accent-080: var(--wui-color-accent-base-080);

        --wui-color-success-100: var(--wui-color-success-base-100);
        --wui-color-success-125: var(--wui-color-success-base-125);

        --wui-color-warning-100: var(--wui-color-warning-base-100);

        --wui-color-error-100: var(--wui-color-error-base-100);
        --wui-color-error-125: var(--wui-color-error-base-125);

        --wui-color-blue-100: var(--wui-color-blue-base-100);
        --wui-color-blue-90: var(--wui-color-blue-base-90);

        --wui-icon-box-bg-error-100: var(--wui-icon-box-bg-error-base-100);
        --wui-icon-box-bg-blue-100: var(--wui-icon-box-bg-blue-base-100);
        --wui-icon-box-bg-success-100: var(--wui-icon-box-bg-success-base-100);
        --wui-icon-box-bg-inverse-100: var(--wui-icon-box-bg-inverse-base-100);

        --wui-all-wallets-bg-100: var(--wui-all-wallets-bg-100);

        --wui-avatar-border: var(--wui-avatar-border-base);

        --wui-thumbnail-border: var(--wui-thumbnail-border-base);

        --wui-wallet-button-bg: var(--wui-wallet-button-bg-base);

        --wui-box-shadow-blue: var(--wui-color-accent-glass-020);
      }

      @supports (background: color-mix(in srgb, white 50%, black)) {
        :root {
          --wui-color-modal-bg: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-modal-bg-base)
          );

          --wui-box-shadow-blue: color-mix(in srgb, var(--wui-color-accent-100) 20%, transparent);

          --wui-color-accent-100: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 100%,
            transparent
          );
          --wui-color-accent-090: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 90%,
            transparent
          );
          --wui-color-accent-080: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 80%,
            transparent
          );
          --wui-color-accent-glass-090: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 90%,
            transparent
          );
          --wui-color-accent-glass-080: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 80%,
            transparent
          );
          --wui-color-accent-glass-020: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 20%,
            transparent
          );
          --wui-color-accent-glass-015: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 15%,
            transparent
          );
          --wui-color-accent-glass-010: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 10%,
            transparent
          );
          --wui-color-accent-glass-005: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 5%,
            transparent
          );
          --wui-color-accent-002: color-mix(
            in srgb,
            var(--wui-color-accent-base-100) 2%,
            transparent
          );

          --wui-color-fg-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-100)
          );
          --wui-color-fg-125: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-125)
          );
          --wui-color-fg-150: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-150)
          );
          --wui-color-fg-175: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-175)
          );
          --wui-color-fg-200: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-200)
          );
          --wui-color-fg-225: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-225)
          );
          --wui-color-fg-250: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-250)
          );
          --wui-color-fg-275: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-275)
          );
          --wui-color-fg-300: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-300)
          );
          --wui-color-fg-325: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-325)
          );
          --wui-color-fg-350: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-fg-350)
          );

          --wui-color-bg-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-100)
          );
          --wui-color-bg-125: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-125)
          );
          --wui-color-bg-150: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-150)
          );
          --wui-color-bg-175: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-175)
          );
          --wui-color-bg-200: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-200)
          );
          --wui-color-bg-225: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-225)
          );
          --wui-color-bg-250: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-250)
          );
          --wui-color-bg-275: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-275)
          );
          --wui-color-bg-300: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-300)
          );
          --wui-color-bg-325: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-325)
          );
          --wui-color-bg-350: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-bg-350)
          );

          --wui-color-success-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-success-base-100)
          );
          --wui-color-success-125: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-success-base-125)
          );

          --wui-color-warning-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-warning-base-100)
          );

          --wui-color-error-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-error-base-100)
          );
          --wui-color-blue-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-blue-base-100)
          );
          --wui-color-blue-90: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-blue-base-90)
          );
          --wui-color-error-125: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-color-error-base-125)
          );

          --wui-icon-box-bg-error-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-icon-box-bg-error-base-100)
          );
          --wui-icon-box-bg-accent-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-icon-box-bg-blue-base-100)
          );
          --wui-icon-box-bg-success-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-icon-box-bg-success-base-100)
          );
          --wui-icon-box-bg-inverse-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-icon-box-bg-inverse-base-100)
          );

          --wui-all-wallets-bg-100: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-all-wallets-bg-100)
          );

          --wui-avatar-border: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-avatar-border-base)
          );

          --wui-thumbnail-border: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-thumbnail-border-base)
          );

          --wui-wallet-button-bg: color-mix(
            in srgb,
            var(--w3m-color-mix) var(--w3m-color-mix-strength),
            var(--wui-wallet-button-bg-base)
          );
        }
      }
    `,light:o`
      :root {
        --w3m-color-mix: ${s(e?.[`--w3m-color-mix`]||`#fff`)};
        --w3m-accent: ${s(In(e,`dark`)[`--w3m-accent`])};
        --w3m-default: #fff;

        --wui-color-modal-bg-base: ${s(In(e,`dark`)[`--w3m-background`])};
        --wui-color-accent-base-100: var(--w3m-accent);

        --wui-color-blueberry-100: hsla(230, 100%, 67%, 1);
        --wui-color-blueberry-090: hsla(231, 76%, 61%, 1);
        --wui-color-blueberry-080: hsla(230, 59%, 55%, 1);
        --wui-color-blueberry-050: hsla(231, 100%, 70%, 0.1);

        --wui-color-fg-100: #e4e7e7;
        --wui-color-fg-125: #d0d5d5;
        --wui-color-fg-150: #a8b1b1;
        --wui-color-fg-175: #a8b0b0;
        --wui-color-fg-200: #949e9e;
        --wui-color-fg-225: #868f8f;
        --wui-color-fg-250: #788080;
        --wui-color-fg-275: #788181;
        --wui-color-fg-300: #6e7777;
        --wui-color-fg-325: #9a9a9a;
        --wui-color-fg-350: #363636;

        --wui-color-bg-100: #141414;
        --wui-color-bg-125: #191a1a;
        --wui-color-bg-150: #1e1f1f;
        --wui-color-bg-175: #222525;
        --wui-color-bg-200: #272a2a;
        --wui-color-bg-225: #2c3030;
        --wui-color-bg-250: #313535;
        --wui-color-bg-275: #363b3b;
        --wui-color-bg-300: #3b4040;
        --wui-color-bg-325: #252525;
        --wui-color-bg-350: #ffffff;

        --wui-color-success-base-100: #26d962;
        --wui-color-success-base-125: #30a46b;

        --wui-color-warning-base-100: #f3a13f;

        --wui-color-error-base-100: #f25a67;
        --wui-color-error-base-125: #df4a34;

        --wui-color-blue-base-100: rgba(102, 125, 255, 1);
        --wui-color-blue-base-90: rgba(102, 125, 255, 0.9);

        --wui-color-success-glass-001: rgba(38, 217, 98, 0.01);
        --wui-color-success-glass-002: rgba(38, 217, 98, 0.02);
        --wui-color-success-glass-005: rgba(38, 217, 98, 0.05);
        --wui-color-success-glass-010: rgba(38, 217, 98, 0.1);
        --wui-color-success-glass-015: rgba(38, 217, 98, 0.15);
        --wui-color-success-glass-020: rgba(38, 217, 98, 0.2);
        --wui-color-success-glass-025: rgba(38, 217, 98, 0.25);
        --wui-color-success-glass-030: rgba(38, 217, 98, 0.3);
        --wui-color-success-glass-060: rgba(38, 217, 98, 0.6);
        --wui-color-success-glass-080: rgba(38, 217, 98, 0.8);

        --wui-color-success-glass-reown-020: rgba(48, 164, 107, 0.2);

        --wui-color-warning-glass-reown-020: rgba(243, 161, 63, 0.2);

        --wui-color-error-glass-001: rgba(242, 90, 103, 0.01);
        --wui-color-error-glass-002: rgba(242, 90, 103, 0.02);
        --wui-color-error-glass-005: rgba(242, 90, 103, 0.05);
        --wui-color-error-glass-010: rgba(242, 90, 103, 0.1);
        --wui-color-error-glass-015: rgba(242, 90, 103, 0.15);
        --wui-color-error-glass-020: rgba(242, 90, 103, 0.2);
        --wui-color-error-glass-025: rgba(242, 90, 103, 0.25);
        --wui-color-error-glass-030: rgba(242, 90, 103, 0.3);
        --wui-color-error-glass-060: rgba(242, 90, 103, 0.6);
        --wui-color-error-glass-080: rgba(242, 90, 103, 0.8);

        --wui-color-error-glass-reown-020: rgba(223, 74, 52, 0.2);

        --wui-color-gray-glass-001: rgba(255, 255, 255, 0.01);
        --wui-color-gray-glass-002: rgba(255, 255, 255, 0.02);
        --wui-color-gray-glass-005: rgba(255, 255, 255, 0.05);
        --wui-color-gray-glass-010: rgba(255, 255, 255, 0.1);
        --wui-color-gray-glass-015: rgba(255, 255, 255, 0.15);
        --wui-color-gray-glass-020: rgba(255, 255, 255, 0.2);
        --wui-color-gray-glass-025: rgba(255, 255, 255, 0.25);
        --wui-color-gray-glass-030: rgba(255, 255, 255, 0.3);
        --wui-color-gray-glass-060: rgba(255, 255, 255, 0.6);
        --wui-color-gray-glass-080: rgba(255, 255, 255, 0.8);
        --wui-color-gray-glass-090: rgba(255, 255, 255, 0.9);

        --wui-color-dark-glass-100: rgba(42, 42, 42, 1);

        --wui-icon-box-bg-error-base-100: #3c2426;
        --wui-icon-box-bg-blue-base-100: #20303f;
        --wui-icon-box-bg-success-base-100: #1f3a28;
        --wui-icon-box-bg-inverse-base-100: #243240;

        --wui-all-wallets-bg-100: #222b35;

        --wui-avatar-border-base: #252525;

        --wui-thumbnail-border-base: #252525;

        --wui-wallet-button-bg-base: var(--wui-color-bg-125);

        --w3m-card-embedded-shadow-color: rgb(17 17 18 / 25%);
      }
    `,dark:o`
      :root {
        --w3m-color-mix: ${s(e?.[`--w3m-color-mix`]||`#000`)};
        --w3m-accent: ${s(In(e,`light`)[`--w3m-accent`])};
        --w3m-default: #000;

        --wui-color-modal-bg-base: ${s(In(e,`light`)[`--w3m-background`])};
        --wui-color-accent-base-100: var(--w3m-accent);

        --wui-color-blueberry-100: hsla(231, 100%, 70%, 1);
        --wui-color-blueberry-090: hsla(231, 97%, 72%, 1);
        --wui-color-blueberry-080: hsla(231, 92%, 74%, 1);

        --wui-color-fg-100: #141414;
        --wui-color-fg-125: #2d3131;
        --wui-color-fg-150: #474d4d;
        --wui-color-fg-175: #636d6d;
        --wui-color-fg-200: #798686;
        --wui-color-fg-225: #828f8f;
        --wui-color-fg-250: #8b9797;
        --wui-color-fg-275: #95a0a0;
        --wui-color-fg-300: #9ea9a9;
        --wui-color-fg-325: #9a9a9a;
        --wui-color-fg-350: #d0d0d0;

        --wui-color-bg-100: #ffffff;
        --wui-color-bg-125: #f5fafa;
        --wui-color-bg-150: #f3f8f8;
        --wui-color-bg-175: #eef4f4;
        --wui-color-bg-200: #eaf1f1;
        --wui-color-bg-225: #e5eded;
        --wui-color-bg-250: #e1e9e9;
        --wui-color-bg-275: #dce7e7;
        --wui-color-bg-300: #d8e3e3;
        --wui-color-bg-325: #f3f3f3;
        --wui-color-bg-350: #202020;

        --wui-color-success-base-100: #26b562;
        --wui-color-success-base-125: #30a46b;

        --wui-color-warning-base-100: #f3a13f;

        --wui-color-error-base-100: #f05142;
        --wui-color-error-base-125: #df4a34;

        --wui-color-blue-base-100: rgba(102, 125, 255, 1);
        --wui-color-blue-base-90: rgba(102, 125, 255, 0.9);

        --wui-color-success-glass-001: rgba(38, 181, 98, 0.01);
        --wui-color-success-glass-002: rgba(38, 181, 98, 0.02);
        --wui-color-success-glass-005: rgba(38, 181, 98, 0.05);
        --wui-color-success-glass-010: rgba(38, 181, 98, 0.1);
        --wui-color-success-glass-015: rgba(38, 181, 98, 0.15);
        --wui-color-success-glass-020: rgba(38, 181, 98, 0.2);
        --wui-color-success-glass-025: rgba(38, 181, 98, 0.25);
        --wui-color-success-glass-030: rgba(38, 181, 98, 0.3);
        --wui-color-success-glass-060: rgba(38, 181, 98, 0.6);
        --wui-color-success-glass-080: rgba(38, 181, 98, 0.8);

        --wui-color-success-glass-reown-020: rgba(48, 164, 107, 0.2);

        --wui-color-warning-glass-reown-020: rgba(243, 161, 63, 0.2);

        --wui-color-error-glass-001: rgba(240, 81, 66, 0.01);
        --wui-color-error-glass-002: rgba(240, 81, 66, 0.02);
        --wui-color-error-glass-005: rgba(240, 81, 66, 0.05);
        --wui-color-error-glass-010: rgba(240, 81, 66, 0.1);
        --wui-color-error-glass-015: rgba(240, 81, 66, 0.15);
        --wui-color-error-glass-020: rgba(240, 81, 66, 0.2);
        --wui-color-error-glass-025: rgba(240, 81, 66, 0.25);
        --wui-color-error-glass-030: rgba(240, 81, 66, 0.3);
        --wui-color-error-glass-060: rgba(240, 81, 66, 0.6);
        --wui-color-error-glass-080: rgba(240, 81, 66, 0.8);

        --wui-color-error-glass-reown-020: rgba(223, 74, 52, 0.2);

        --wui-icon-box-bg-error-base-100: #f4dfdd;
        --wui-icon-box-bg-blue-base-100: #d9ecfb;
        --wui-icon-box-bg-success-base-100: #daf0e4;
        --wui-icon-box-bg-inverse-base-100: #dcecfc;

        --wui-all-wallets-bg-100: #e8f1fa;

        --wui-avatar-border-base: #f3f4f4;

        --wui-thumbnail-border-base: #eaefef;

        --wui-wallet-button-bg-base: var(--wui-color-bg-125);

        --wui-color-gray-glass-001: rgba(0, 0, 0, 0.01);
        --wui-color-gray-glass-002: rgba(0, 0, 0, 0.02);
        --wui-color-gray-glass-005: rgba(0, 0, 0, 0.05);
        --wui-color-gray-glass-010: rgba(0, 0, 0, 0.1);
        --wui-color-gray-glass-015: rgba(0, 0, 0, 0.15);
        --wui-color-gray-glass-020: rgba(0, 0, 0, 0.2);
        --wui-color-gray-glass-025: rgba(0, 0, 0, 0.25);
        --wui-color-gray-glass-030: rgba(0, 0, 0, 0.3);
        --wui-color-gray-glass-060: rgba(0, 0, 0, 0.6);
        --wui-color-gray-glass-080: rgba(0, 0, 0, 0.8);
        --wui-color-gray-glass-090: rgba(0, 0, 0, 0.9);

        --wui-color-dark-glass-100: rgba(233, 233, 233, 1);

        --w3m-card-embedded-shadow-color: rgb(224 225 233 / 25%);
      }
    `}}var $r=o`
  *,
  *::after,
  *::before,
  :host {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
    font-style: normal;
    text-rendering: optimizeSpeed;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    -webkit-tap-highlight-color: transparent;
    font-family: var(--wui-font-family);
    backface-visibility: hidden;
  }
`,ei=o`
  button,
  a {
    cursor: pointer;
    display: flex;
    justify-content: center;
    align-items: center;
    position: relative;
    transition:
      color var(--wui-duration-lg) var(--wui-ease-out-power-1),
      background-color var(--wui-duration-lg) var(--wui-ease-out-power-1),
      border var(--wui-duration-lg) var(--wui-ease-out-power-1),
      border-radius var(--wui-duration-lg) var(--wui-ease-out-power-1),
      box-shadow var(--wui-duration-lg) var(--wui-ease-out-power-1);
    will-change: background-color, color, border, box-shadow, border-radius;
    outline: none;
    border: none;
    column-gap: var(--wui-spacing-3xs);
    background-color: transparent;
    text-decoration: none;
  }

  wui-flex {
    transition: border-radius var(--wui-duration-lg) var(--wui-ease-out-power-1);
    will-change: border-radius;
  }

  button:disabled > wui-wallet-image,
  button:disabled > wui-all-wallets-image,
  button:disabled > wui-network-image,
  button:disabled > wui-image,
  button:disabled > wui-transaction-visual,
  button:disabled > wui-logo {
    filter: grayscale(1);
  }

  @media (hover: hover) and (pointer: fine) {
    button:hover:enabled {
      background-color: var(--wui-color-gray-glass-005);
    }

    button:active:enabled {
      background-color: var(--wui-color-gray-glass-010);
    }
  }

  button:disabled > wui-icon-box {
    opacity: 0.5;
  }

  input {
    border: none;
    outline: none;
    appearance: none;
  }
`,ti=o`
  .wui-color-inherit {
    color: var(--wui-color-inherit);
  }

  .wui-color-accent-100 {
    color: var(--wui-color-accent-100);
  }

  .wui-color-error-100 {
    color: var(--wui-color-error-100);
  }

  .wui-color-blue-100 {
    color: var(--wui-color-blue-100);
  }

  .wui-color-blue-90 {
    color: var(--wui-color-blue-90);
  }

  .wui-color-error-125 {
    color: var(--wui-color-error-125);
  }

  .wui-color-success-100 {
    color: var(--wui-color-success-100);
  }

  .wui-color-success-125 {
    color: var(--wui-color-success-125);
  }

  .wui-color-inverse-100 {
    color: var(--wui-color-inverse-100);
  }

  .wui-color-inverse-000 {
    color: var(--wui-color-inverse-000);
  }

  .wui-color-fg-100 {
    color: var(--wui-color-fg-100);
  }

  .wui-color-fg-200 {
    color: var(--wui-color-fg-200);
  }

  .wui-color-fg-300 {
    color: var(--wui-color-fg-300);
  }

  .wui-color-fg-325 {
    color: var(--wui-color-fg-325);
  }

  .wui-color-fg-350 {
    color: var(--wui-color-fg-350);
  }

  .wui-bg-color-inherit {
    background-color: var(--wui-color-inherit);
  }

  .wui-bg-color-blue-100 {
    background-color: var(--wui-color-accent-100);
  }

  .wui-bg-color-error-100 {
    background-color: var(--wui-color-error-100);
  }

  .wui-bg-color-error-125 {
    background-color: var(--wui-color-error-125);
  }

  .wui-bg-color-success-100 {
    background-color: var(--wui-color-success-100);
  }

  .wui-bg-color-success-125 {
    background-color: var(--wui-color-success-100);
  }

  .wui-bg-color-inverse-100 {
    background-color: var(--wui-color-inverse-100);
  }

  .wui-bg-color-inverse-000 {
    background-color: var(--wui-color-inverse-000);
  }

  .wui-bg-color-fg-100 {
    background-color: var(--wui-color-fg-100);
  }

  .wui-bg-color-fg-200 {
    background-color: var(--wui-color-fg-200);
  }

  .wui-bg-color-fg-300 {
    background-color: var(--wui-color-fg-300);
  }

  .wui-color-fg-325 {
    background-color: var(--wui-color-fg-325);
  }

  .wui-color-fg-350 {
    background-color: var(--wui-color-fg-350);
  }
`,ni={getSpacingStyles(e,t){if(Array.isArray(e))return e[t]?`var(--wui-spacing-${e[t]})`:void 0;if(typeof e==`string`)return`var(--wui-spacing-${e})`},getFormattedDate(e){return new Intl.DateTimeFormat(`en-US`,{month:`short`,day:`numeric`}).format(e)},getHostName(e){try{return new URL(e).hostname}catch{return``}},getTruncateString({string:e,charsStart:t,charsEnd:n,truncate:r}){return e.length<=t+n?e:r===`end`?`${e.substring(0,t)}...`:r===`start`?`...${e.substring(e.length-n)}`:`${e.substring(0,Math.floor(t))}...${e.substring(e.length-Math.floor(n))}`},generateAvatarColors(e){let t=e.toLowerCase().replace(/^0x/iu,``).replace(/[^a-f0-9]/gu,``).substring(0,6).padEnd(6,`0`),n=this.hexToRgb(t),r=getComputedStyle(document.documentElement).getPropertyValue(`--w3m-border-radius-master`),i=100-3*Number(r?.replace(`px`,``)),a=`${i}% ${i}% at 65% 40%`,o=[];for(let e=0;e<5;e+=1){let t=this.tintColor(n,.15*e);o.push(`rgb(${t[0]}, ${t[1]}, ${t[2]})`)}return`
    --local-color-1: ${o[0]};
    --local-color-2: ${o[1]};
    --local-color-3: ${o[2]};
    --local-color-4: ${o[3]};
    --local-color-5: ${o[4]};
    --local-radial-circle: ${a}
   `},hexToRgb(e){let t=parseInt(e,16);return[t>>16&255,t>>8&255,t&255]},tintColor(e,t){let[n,r,i]=e;return[Math.round(n+(255-n)*t),Math.round(r+(255-r)*t),Math.round(i+(255-i)*t)]},isNumber(e){return{number:/^[0-9]+$/u}.number.test(e)},getColorTheme(e){return e||(typeof window<`u`&&window.matchMedia?window.matchMedia(`(prefers-color-scheme: dark)`)?.matches?`dark`:`light`:`dark`)},splitBalance(e){let t=e.split(`.`);return t.length===2?[t[0],t[1]]:[`0`,`00`]},roundNumber(e,t,n){return e.toString().length>=t?Number(e).toFixed(n):e},formatNumberToLocalString(e,t=2){return e===void 0?`0.00`:typeof e==`number`?e.toLocaleString(`en-US`,{maximumFractionDigits:t,minimumFractionDigits:t}):parseFloat(e).toLocaleString(`en-US`,{maximumFractionDigits:t,minimumFractionDigits:t})}};function ri(e,t){let{kind:n,elements:r}=t;return{kind:n,elements:r,finisher(t){customElements.get(e)||customElements.define(e,t)}}}function ii(e,t){return customElements.get(e)||customElements.define(e,t),t}function ai(e){return function(t){return typeof t==`function`?ii(e,t):ri(e,t)}}export{ye as $,O as A,Wt as B,N as C,dr as D,mr as E,C as F,Rt as G,Ht as H,ln as I,xt as J,St as K,cn as L,E as M,rr as N,A as O,D as P,Se as Q,Jt as R,I as S,M as T,Ut as U,Vt as V,zt as W,vt as X,yt as Y,ze as Z,jr as _,Yr as a,f as at,Tr as b,Zr as c,Z as d,v as et,X as f,G as g,Fr as h,ei as i,g as it,ir as j,k,Gr as l,Ir as m,ni as n,_e as nt,$r as o,d as ot,J as p,bt as q,ti as r,h as rt,Xr as s,ai as t,me as tt,$ as u,H as v,_r as w,R as x,B as y,Kt as z};