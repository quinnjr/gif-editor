export interface DragState {
  isDragging: boolean;
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  offsetX: number;
  offsetY: number;
}

export function createDragHandler(
  onMove: (dx: number, dy: number) => void,
  onEnd: () => void,
) {
  let startX = 0;
  let startY = 0;

  function onPointerDown(e: PointerEvent) {
    startX = e.clientX;
    startY = e.clientY;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
  }

  function onPointerMove(e: PointerEvent) {
    onMove(e.clientX - startX, e.clientY - startY);
  }

  function onPointerUp() {
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    onEnd();
  }

  return { onPointerDown };
}
