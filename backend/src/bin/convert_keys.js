const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

function encode(source) {
  if (source.length === 0) return '';
  let res = BigInt(0);
  for (const byte of source) {
    res = res * 256n + BigInt(byte);
  }
  let s = '';
  while (res > 0n) {
    s = ALPHABET[Number(res % 58n)] + s;
    res = res / 58n;
  }
  for (let i = 0; i < source.length && source[i] === 0; i++) {
    s = ALPHABET[0] + s;
  }
  return s;
}

const vps = [41,224,139,14,214,10,174,235,65,220,90,211,52,227,52,37,94,162,254,158,105,20,85,239,112,177,145,8,28,67,57,238,157,43,25,165,237,98,245,188,135,45,186,60,108,223,25,22,6,81,1,215,183,191,80,172,161,228,83,57,117,36,238,214];
const kyc = [105,46,160,183,85,238,77,73,158,86,235,20,21,68,143,236,181,121,192,87,123,119,9,71,16,37,12,101,135,123,150,185,196,109,208,161,31,50,197,76,13,188,157,147,0,142,53,55,100,6,85,213,31,109,116,245,185,101,176,22,105,109,171,164];

console.log("VPS_BASE58:" + encode(vps));
console.log("KYC_BASE58:" + encode(kyc));
