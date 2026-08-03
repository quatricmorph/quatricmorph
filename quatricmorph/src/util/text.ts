// @ts-nocheck
import * as THREE from 'three'
import { FontLoader } from 'three/addons/loaders/FontLoader.js'
import * as typeface from '../assets/droid_sans_regular.typeface.js'

const font = new FontLoader().parse(typeface.data)

export function getText(msg, color = 0x006699, size = 1) {
  const shapes = font.generateShapes(msg, size)
  const geometry = new THREE.ShapeGeometry(shapes)
  geometry.computeBoundingBox()
  const matLite = new THREE.MeshBasicMaterial({
    color: color,
    side: THREE.DoubleSide
  })
  return new THREE.Mesh(geometry, matLite)
}
