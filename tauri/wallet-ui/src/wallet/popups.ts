export function trackWalletPopups(owner: Window = window): () => void {
  const original = owner.open;
  const descriptor = Object.getOwnPropertyDescriptor(owner, "open");
  const children = new Set<Window>();
  const open: Window["open"] = (...args) => {
    const child = original.apply(owner, args);
    if (child && child !== owner) children.add(child);
    return child;
  };
  const closeChildren = () => {
    for (const child of children) {
      try { child.close(); } catch { /* The provider may have already closed it. */ }
    }
    children.clear();
  };
  Object.defineProperty(owner, "open", { configurable: true, enumerable: descriptor?.enumerable ?? true, writable: true, value: open });
  owner.addEventListener("pagehide", closeChildren);
  return () => {
    owner.removeEventListener("pagehide", closeChildren);
    if (owner.open === open) {
      if (descriptor) Object.defineProperty(owner, "open", descriptor);
      else Reflect.deleteProperty(owner, "open");
    }
    closeChildren();
  };
}
