function capitalize(str) {
  return str.charAt(0).toUpperCase() + str.slice(1);
}


function truncateString(str, num) {
  if (str.length <= num) {
    return str
  }
  return str.slice(0, num) + '...'
}

module.exports = { capitalize, truncateString };