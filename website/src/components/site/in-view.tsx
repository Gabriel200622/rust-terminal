"use client";

import { useEffect, useRef, useState } from "react";

/**
 * Marks its content while it is on screen, so looping motion inside it runs
 * only when someone can see it.
 */
export function InView({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  const element = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    if (!element.current) return;
    const observer = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting), {
      threshold: 0.3,
    });
    observer.observe(element.current);
    return () => observer.disconnect();
  }, []);
  return (
    <div ref={element} data-inview={visible} className={className}>
      {children}
    </div>
  );
}
