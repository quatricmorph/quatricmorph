import React from 'react'
import Navbar from './components/Navbar'
import Hero from './components/Hero'
import BackgroundCubes from './components/BackgroundCubes'

export default function App(){
  return (
    <div className="min-h-screen relative overflow-hidden bg-qm-gradient text-white">
      <BackgroundCubes />
      <Navbar />
      <main className="flex items-center justify-center h-[80vh] px-6">
        <Hero />
      </main>
      <footer className="absolute bottom-4 w-full text-center text-sm text-qm-silver/60">© QuatricMorph</footer>
    </div>
  )
}