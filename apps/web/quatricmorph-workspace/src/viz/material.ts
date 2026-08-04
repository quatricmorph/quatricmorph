// @ts-nocheck
import * as THREE from 'three'

const TEXTURE = new THREE.TextureLoader().load('/assets/ball.png')

export const MATERIAL = new THREE.ShaderMaterial({
  // Keep GLSL1 so texture2D / gl_FragColor work on three r163+
  glslVersion: THREE.GLSL1,
  uniforms: {
    color: { value: new THREE.Color(0xffffff) },
    pointTexture: { value: TEXTURE },
    mag: { value: 1.0 },
  },

  vertexShader: `
  uniform float mag;
  attribute float pointSize;
  attribute vec4 pointColor;
  varying vec4 vColor;

  void main() {
    vColor = pointColor;
    vec4 mvPosition = modelViewMatrix * vec4( position, 1.0 );
    gl_PointSize = mag * pointSize / -mvPosition.z;
    gl_Position = projectionMatrix * mvPosition;
  }
`,

  fragmentShader: `
  uniform vec3 color;
  uniform sampler2D pointTexture;
  varying vec4 vColor;

  void main() {
    vec4 outColor = texture2D( pointTexture, gl_PointCoord );
    if ( outColor.a < 0.5 ) discard;
    gl_FragColor = outColor * vec4( color * vColor.xyz, 1.0 );
  }`,
})
