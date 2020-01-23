import React from "react";

// React children can really be a lot of different types, so we just use any here.
export function getSlots(children: any) {
  let slots: { [key: string]: any[] } = {};
  let orphans: any[] = [];
  React.Children.forEach(children, (child: any) => {
    if (!child) return;
    if (!child.props || !child.props["data-slot"]) {
      orphans.push(child);
    } else {
      let slot = child.props["data-slot"];
      if (slot) {
        if (slots[slot]) {
          slots[slot].push(child);
        } else {
          slots[slot] = [child];
        }
      }
    }
  });
  return { slots, orphans };
}
