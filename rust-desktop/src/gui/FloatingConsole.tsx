import { useEffect, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

// The native window hosts a portal in the same React tree: audio, timers and
// settings have one owner and keep running while the console is hidden.
export function FloatingConsole({ children, onWindow }: {
  children: ReactNode;
  onWindow: (host: Window | null) => void;
}) {
  const [container, setContainer] = useState<HTMLElement | null>(null);
  useEffect(() => {
    if (!window.desktopPetBridge) return;
    const host = window.open('about:blank', 'pet-console', 'width=470,height=780');
    if (!host) return;
    const doc = host.document;
    doc.title = '桌宠节奏师 · 控制台';
    doc.documentElement.lang = 'zh-CN';
    const base = doc.createElement('base');
    base.href = document.baseURI;
    doc.head.append(base);
    const styles = [...document.querySelectorAll('style,link[rel="stylesheet"]')]
      .map((node) => node.cloneNode(true));
    doc.head.append(...styles);
    // Keep development styles current after a Vite style update.
    const observer = new MutationObserver(() => {
      doc.querySelectorAll('[data-console-style]').forEach((node) => node.remove());
      document.querySelectorAll('style,link[rel="stylesheet"]').forEach((node) => {
        const clone = node.cloneNode(true) as HTMLElement;
        clone.dataset.consoleStyle = 'true';
        doc.head.append(clone);
      });
      styles.forEach((node) => node.parentNode?.removeChild(node));
    });
    observer.observe(document.head, { childList: true, subtree: true, characterData: true });
    doc.body.className = 'console-body';
    const root = doc.createElement('div');
    root.className = 'console-root';
    doc.body.append(root);
    setContainer(root);
    onWindow(host);
    return () => {
      observer.disconnect();
      onWindow(null);
      root.remove();
      base.remove();
      styles.forEach((node) => node.parentNode?.removeChild(node));
      doc.querySelectorAll('[data-console-style]').forEach((node) => node.remove());
    };
  }, [onWindow]);
  if (!window.desktopPetBridge) return <div className="browser-console">{children}</div>;
  return container ? createPortal(children, container) : null;
}
