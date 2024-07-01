import * as crypto from 'crypto';



function hashPublicKey(pub_key_x: number[], pub_key_y: number[]): String {
    if (pub_key_x.length !== 32 || pub_key_y.length !== 32) {
        throw new Error('pub_key_x and pub_key_y size need to be 32bytes.');
    }
    const publicKey = Buffer.concat([Buffer.from(pub_key_x), Buffer.from(pub_key_y)]);
    const hash = crypto.createHash('sha256').update(publicKey).digest();
    const result = hash.slice(-20);
    const hexResult = Array.from(result).map(byte => byte.toString(16).padStart(2, '0'));
    
    return hexResult.join("");
}

// let pub_key_x = [201,91,99,172,65,154,80,154,189,195,194,210,62,219,224,36,43,134,143,236,137,178,121,35,112,146,103,238,37,100,145,26];
// let pub_key_y = [169,8,63,83,58,93,192,39,114,115,138,176,56,254,162,127,93,19,156,93,51,9,194,161,253,10,203,128,171,254,255,83];
// let pub_key_x = [15,206,241,12,21,160,54,11,79,72,44,109,43,45,101,54,210,243,13,236,51,33,47,66,187,38,60,249,64,70,37,252];
// let pub_key_y = [43,221,114,86,240,184,224,51,237,41,173,85,147,130,225,150,159,150,44,23,57,92,82,37,27,40,69,123,252,224,3,197];
// let pub_key_x = [35,250,194,235,47,86,159,70,36,137,145,195,83,245,203,137,12,28,43,171,167,226,44,90,199,107,235,0,229,229,133,67];
// let pub_key_y = [84,107,152,100,72,140,98,57,186,30,76,187,129,194,209,158,96,37,254,211,60,198,27,227,167,247,204,51,145,53,120,95];
let pub_key_x = [10,139,43,102,182,222,131,127,94,44,137,46,114,246,188,198,153,38,51,220,104,189,146,100,20,183,186,135,40,241,63,90];
let pub_key_y = [248,109,104,228,138,216,189,114,45,18,108,136,174,69,16,115,225,68,38,193,19,153,45,106,117,46,233,180,209,239,182,202];

console.log(hashPublicKey(pub_key_x, pub_key_y));
process.exit();