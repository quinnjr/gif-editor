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
  // Rust serialises `Option<T>` fields as explicit `null`, so every
  // optional field here must admit `null` in addition to being absent.
  text?: string | null;
  font_family?: string | null;
  font_size?: number | null;
  color?: [number, number, number, number] | null;
  stroke?: Stroke | null;
  text_align?: string | null;
  max_width?: number | null;
  source_width?: number | null;
  source_height?: number | null;
  source_path?: string | null;
  is_animated?: boolean | null;
  intensity?: number | null;
  scale?: number | null;
  pulse_speed?: number | null;
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
  // `null` is accepted on the wire: for stroke/max_width the Rust side uses
  // the double-Option pattern (null = clear the field); for the remaining
  // Option fields serde deserialises null as None (no change).
  text?: string | null;
  font_family?: string | null;
  font_size?: number | null;
  color?: [number, number, number, number] | null;
  stroke?: Stroke | null;
  text_align?: string | null;
  max_width?: number | null;
  keyframes?: Keyframe[];
  intensity?: number | null;
  scale?: number | null;
  pulse_speed?: number | null;
}

export type ExportFormat = 'Gif' | 'Mp4' | 'WebM' | 'Png' | 'Jpeg' | 'WebP';

export interface ExportSettings {
  format: ExportFormat;
  quality: number;
  resize?: [number, number] | null;
  frame_index?: number | null;
}
