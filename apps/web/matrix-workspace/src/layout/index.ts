export {
  GridRuler3D,
  DEFAULT_GRID_RULER,
  GRID_SNAP_TOLERANCE,
  gridRulerFromParams,
  DEFAULT_GRID_RULED_LINES,
  DEFAULT_MARGIN_GRID,
  snapToGrid,
  isGridSnapped,
  cellCenterLocal,
  localTensorExtent,
  mulVolumeExtent,
  placeOperands,
  cameraPresetPose,
  marginGridFromParams,
} from './grid-ruler.js'
export type {
  GridRuler3DConfig,
  Vec3,
  GridRuledLinesConfig,
  MarginGridConfig,
  PlacementHints,
  PlaneTransform,
  TensorExtents,
  CameraPreset,
} from './grid-ruler.js'

export { buildTensorFrame, frameContainsPoint } from './tensor-frame.js'
export type { TensorMarginFrame } from './tensor-frame.js'
