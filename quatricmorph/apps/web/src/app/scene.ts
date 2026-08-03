// @ts-nocheck
import * as THREE from 'three'
import { OrbitControls } from 'three/addons/controls/OrbitControls.js'

export function createScene() {
  const aspect = () => window.innerWidth / window.innerHeight
  const fov = () => 45 / Math.min(1, aspect())

  const camera = new THREE.PerspectiveCamera(fov(), aspect(), 5, 10000)
  const pointer = new THREE.Vector2(-1, -1)
  const raycaster = new THREE.Raycaster()
  raycaster.params.Points.threshold = 0
  raycaster.setFromCamera(pointer, camera)

  const scene = new THREE.Scene()

  const renderer = new THREE.WebGLRenderer({ antialias: true })
  renderer.setPixelRatio(window.devicePixelRatio)
  renderer.setSize(window.innerWidth, window.innerHeight)
  document.getElementById('container').appendChild(renderer.domElement)

  const render_info = renderer.info.memory

  const getContext = () => ({
    raycaster,
    camera,
    pointer,
  })

  const orbit = new OrbitControls(camera, renderer.domElement)
  orbit.keys = { LEFT: orbit.keys.LEFT, RIGHT: orbit.keys.RIGHT }
  orbit.zoomSpeed = 0.2
  orbit.listenToKeyEvents(window)

  const viewState = () => ({
    ...camera.position,
    target: { ...orbit.target },
  })

  return {
    aspect,
    fov,
    camera,
    pointer,
    raycaster,
    scene,
    renderer,
    render_info,
    getContext,
    orbit,
    viewState,
  }
}
