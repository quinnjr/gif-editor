export interface GifMetadata {
  frame_count: number;
  width: number;
  height: number;
  delays: number[];
}

export interface Stroke {
  color: [number, number, number, number];
  width: number;
}

export interface LayerInfo {
  id: string;
  name: string;
  layer_type: 'image' | 'text';
  position: [number, number];
  scale_x: number;
  scale_y: number;
  skew_x: number;
  skew_y: number;
  opacity: number;
  frame_range: [number, number];
  visible: boolean;
  text?: string;
  font_family?: string;
  font_size?: number;
  color?: [number, number, number, number];
  stroke?: Stroke | null;
  source_width?: number;
  source_height?: number;
  source_path?: string;
}

export interface LayerUpdate {
  name?: string;
  position?: [number, number];
  scale_x?: number;
  scale_y?: number;
  skew_x?: number;
  skew_y?: number;
  opacity?: number;
  frame_range?: [number, number];
  visible?: boolean;
  text?: string;
  font_family?: string;
  font_size?: number;
  color?: [number, number, number, number];
  stroke?: Stroke | null;
}

export type ExportFormat = 'Gif' | 'Mp4' | 'WebM';

export interface ExportSettings {
  format: ExportFormat;
  quality: number;
  resize?: [number, number] | null;
}
