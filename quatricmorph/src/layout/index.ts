export {
  DEFAULT_GRID_RULED_LINES,
  DEFAULT_MARGIN_GRID,
  snapToGrid,
  isGridSnapped,
  cellCenterLocal,
  localTensorExtent,
  mulVolumeExtent,
  placeOperands,
  cameraPresetPose,
  gridRuledLinesFromParams,
  marginGridFromParams,
} from './grid-ruled-lines.js'
export type {
  Vec3,
  GridRuledLinesConfig,
  MarginGridConfig,
  PlacementHints,
  PlaneTransform,
  TensorExtents,
  CameraPreset,
} from './grid-ruled-lines.js'

export { buildTensorFrame, frameContainsPoint } from './tensor-frame.js'
export type { TensorMarginFrame } from './tensor-frame.js'
