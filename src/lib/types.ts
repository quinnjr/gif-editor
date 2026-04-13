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

export interface Keyframe {
  frame: number;
  position: [number, number];
  opacity: number;
}

export interface LayerInfo {
  id: string;
  name: string;
  layer_type: 'image' | 'text' | 'flare';
  position: [number, number];
  scale_x: number;
  scale_y: number;
  skew_x: number;
  skew_y: number;
  rotation: number;
  opacity: number;
  frame_range: [number, number];
  visible: boolean;
  text?: string;
  font_family?: string;
  font_size?: number;
  color?: [number, number, number, number];
  stroke?: Stroke | null;
  text_align?: string;
  max_width?: number | null;
  source_width?: number;
  source_height?: number;
  source_path?: string;
  intensity?: number;
  scale?: number;
  pulse_speed?: number;
  keyframes: Keyframe[];
}

export interface LayerUpdate {
  name?: string;
  position?: [number, number];
  scale_x?: number;
  scale_y?: number;
  skew_x?: number;
  skew_y?: number;
  rotation?: number;
  opacity?: number;
  frame_range?: [number, number];
  visible?: boolean;
  text?: string;
  font_family?: string;
  font_size?: number;
  color?: [number, number, number, number];
  stroke?: Stroke | null;
  text_align?: string;
  max_width?: number | null;
  keyframes?: Keyframe[];
  intensity?: number;
  scale?: number;
  pulse_speed?: number;
}

export type ExportFormat = 'Gif' | 'Mp4' | 'WebM' | 'Png' | 'Jpeg' | 'WebP';

export interface ExportSettings {
  format: ExportFormat;
  quality: number;
  resize?: [number, number] | null;
  frame_index?: number | null;
}
