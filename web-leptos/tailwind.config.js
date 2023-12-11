module.exports = {
  content: {
    relative: true,
    files: ["./src/**/*.rs", "index.html"],
  },
  darkMode: "media", // 'media' or 'class'
  theme: {
    colors: {
      red: '#E30022',
      black: '#353839',
      gray: '#36454F',
      cinnabar: '#E44D2E'
    },
    fontFamily: {
      'serif': ['"Rosarivo"', 'cursive']
    }
  },
  variants: {
  },
  plugins: [],
};