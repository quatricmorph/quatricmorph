import { useState } from 'react'
import './App.css'

function App() {
  const [count, setCount] = useState(0)

  return (
    <>
      <div className="container">
        <h1>Quatricmorph Viewer</h1>
        <p>Welcome to the Quatricmorph visualization platform</p>
        <button onClick={() => setCount(count + 1)}>
          Count: {count}
        </button>
        <div id="canvas-container"></div>
      </div>
    </>
  )
}

export default App
