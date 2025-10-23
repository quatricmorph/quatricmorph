module.exports = {
  content: [
    './index.html',
    './src/**/*.{js,jsx,ts,tsx,vue}'
  ],
  theme: {
    extend: {
      colors: {
        'qm-deep': '#071428',
        'qm-metal': '#0f4a78',
        'qm-silver': '#aeb6c1'
      },
      backgroundImage: {
        'qm-gradient': 'linear-gradient(135deg, #030412 0%, #071428 45%, #050b14 100%)'
      }
    }
  },
  plugins: [],
}