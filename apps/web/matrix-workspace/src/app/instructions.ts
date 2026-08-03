// @ts-nocheck

export function setupInstructions() {
  const instr = document.getElementById('instructions')
  const instr_content = document.getElementById('instructions-content')
  const min_content = document.getElementById('minimized')
  const min_button = document.getElementById('minimize')
  const max_button = document.getElementById('maximize')

  const show = () => {
    instr_content.style.display = 'block'
    min_content.style.display = 'none'
    min_button.style.display = 'block'
    max_button.style.display = 'none'
  }

  const hide = () => {
    instr_content.style.display = 'none'
    min_content.style.display = 'block'
    min_button.style.display = 'none'
    max_button.style.display = 'block'
  }

  min_button.onclick = hide
  instr_content.onclick = hide
  max_button.onclick = show

  instr.style.display = 'block'
  show()
}
