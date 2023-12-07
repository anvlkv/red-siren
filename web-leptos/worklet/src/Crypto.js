import grv from 'polyfill-crypto.getrandomvalues';

(function (window) {
  "use strict";
  const crypto = {
    getRandomValues: grv
  };
  globalThis.crypto = globalThis.crypto || crypto
  if (!window["crypto"]) window["crypto"] = crypto;
})(
  typeof globalThis == "" + void 0
    ? typeof global == "" + void 0
      ? typeof self == "" + void 0
        ? this
        : self
      : global
    : globalThis
);
