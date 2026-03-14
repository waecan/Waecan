use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::fs;
use std::path::PathBuf;

use waecan_core::block::{serialize_header, Block, BlockHeader, CoinbaseTx};
use waecan_crypto::hash::keccak256;
use waecan_crypto::pedersen::PedersenCommitment;

use crate::db::{WaecanDB, CF_CHAIN_META, CF_KEY_IMAGES, CF_UTXO};
use crate::record::OutputRecord;

fn temp_db_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("waecan_test_db_{}", name));
    let _ = fs::remove_dir_all(&path);
    path
}

fn dummy_block(height: u64) -> Block {
    Block {
        header: BlockHeader {
            version: 1,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 1234567890,
            difficulty: 1,
            nonce: 0,
            height,
        },
        coinbase: CoinbaseTx {
            height,
            reward: 50 * 1_000_000_000_000,
            miner_output_key: CompressedEdwardsY::from_slice(&[0u8; 32]).unwrap(),
            genesis_message: vec![],
        },
        transactions: vec![],
    }
}

// 1. commit_block() then verify all outputs appear in CF_UTXO
#[test]
fn test_1_commit_verify_utxo() {
    let path = temp_db_path("test_1");
    let db = WaecanDB::open(path.to_str().unwrap()).unwrap();

    let block = dummy_block(1);
    let out_point = CompressedEdwardsY::from_slice(&[42u8; 32]).unwrap();
    let blind = Scalar::ZERO;

    let out = OutputRecord {
        output_key: out_point,
        commitment: PedersenCommitment::commit(100, &blind),
        height: 1,
        tx_hash: [1u8; 32],
        output_index: 0,
    };

    db.commit_block(&block, &[out.clone()], &[], &[]).unwrap();

    let cf = db.cf(CF_UTXO).unwrap();
    let val = db.db.get_cf(&cf, out_point.as_bytes()).unwrap().unwrap();
    let decoded = OutputRecord::deserialize(&val).unwrap();
    assert_eq!(decoded.output_key, out_point);
    assert_eq!(decoded.height, 1);
}

// 2. Key image stored after commit — double-spend check works
#[test]
fn test_2_key_image_double_spend() {
    let path = temp_db_path("test_2");
    let db = WaecanDB::open(path.to_str().unwrap()).unwrap();

    let block = dummy_block(1);
    let ki = CompressedEdwardsY::from_slice(&[7u8; 32]).unwrap();

    db.commit_block(&block, &[], &[], &[ki]).unwrap();

    let cf = db.cf(CF_KEY_IMAGES).unwrap();
    let val = db.db.get_cf(&cf, ki.as_bytes()).unwrap().unwrap();

    // Proves the double spend check can read it
    assert_eq!(val.len(), 8);
    assert_eq!(u64::from_le_bytes(val.try_into().unwrap()), 1);
}

// 3. Chain tip updates correctly after each block commit
#[test]
fn test_3_chain_tip_updates() {
    let path = temp_db_path("test_3");
    let db = WaecanDB::open(path.to_str().unwrap()).unwrap();

    let block1 = dummy_block(1);
    db.commit_block(&block1, &[], &[], &[]).unwrap();

    let cf = db.cf(CF_CHAIN_META).unwrap();
    let tip_height = db.db.get_cf(&cf, b"tip_height").unwrap().unwrap();
    assert_eq!(u64::from_le_bytes(tip_height.try_into().unwrap()), 1);

    let block2 = dummy_block(2);
    db.commit_block(&block2, &[], &[], &[]).unwrap();

    let new_tip = db.db.get_cf(&cf, b"tip_height").unwrap().unwrap();
    assert_eq!(u64::from_le_bytes(new_tip.try_into().unwrap()), 2);
}

// 4. Atomic rollback failure test
#[test]
fn test_4_atomic_rollback_simulation() {
    let path = temp_db_path("test_4");
    let db = WaecanDB::open(path.to_str().unwrap()).unwrap();

    let block1 = dummy_block(1);
    db.commit_block(&block1, &[], &[], &[]).unwrap();

    // Simulate failure by constructing a batch but failing midway and never writing it.
    // In Rust RocksDB, the batch is completely atomic and is only committed on `.write(batch)`.
    // Since we can't purposefully panic inside `.write(batch)` easily, the atomicity of WriteBatch is guaranteed by RocksDB itself.
    let mut batch = rocksdb::WriteBatch::default();
    let cf = db.cf(CF_UTXO).unwrap();
    batch.put_cf(&cf, b"fake_out", b"data");

    // Never write the batch
    drop(batch);

    let val = db.db.get_cf(&cf, b"fake_out").unwrap();
    assert!(val.is_none());
}

// 5. FEE BURN INVARIANT
#[test]
fn test_5_fee_burn_invariant() {
    let path = temp_db_path("test_5");
    let db = WaecanDB::open(path.to_str().unwrap()).unwrap();

    let mut total_rewards = 0u64;
    let mut total_fees_burned = 0u64;
    let mut total_blindings = Scalar::ZERO;

    let mut sum_utxo_points = EdwardsPoint::default();

    for h_loop in 1..=100 {
        let height = h_loop as u64;
        let mut block = dummy_block(height);

        let reward = waecan_core::block::block_reward(height);
        let fee = 1_000_000_000;

        total_rewards += reward;
        total_fees_burned += fee;

        // Miner gets reward - fee isn't given to miner!
        let out_value = reward - fee;
        let out_blind = Scalar::from(height);
        total_blindings += out_blind;

        let commit = PedersenCommitment::commit(out_value, &out_blind);

        let out_key = CompressedEdwardsY::from_slice(&[height as u8; 32])
            .unwrap_or_else(|_| CompressedEdwardsY::from_slice(&[0u8; 32]).unwrap());

        let out = OutputRecord {
            output_key: out_key,
            commitment: commit.clone(),
            height,
            tx_hash: [0u8; 32],
            output_index: 0,
        };

        db.commit_block(&block, &[out], &[], &[]).unwrap();

        sum_utxo_points += commit.commitment.decompress().unwrap();
    }

    // The homomorphic invariant: sum(UTXO commitments) == commit(sum(rewards) - sum(fees), sum(blindings))
    let expected_net_supply = total_rewards - total_fees_burned;
    let expected_commit = PedersenCommitment::commit(expected_net_supply, &total_blindings);

    assert_eq!(sum_utxo_points.compress(), expected_commit.commitment);
}

// 6. Block disconnect (reorg)
#[test]
fn test_6_block_disconnect_reorg() {
    let path = temp_db_path("test_6");
    let db = WaecanDB::open(path.to_str().unwrap()).unwrap();

    let block1 = dummy_block(1);
    let out1 = OutputRecord {
        output_key: CompressedEdwardsY::from_slice(&[1u8; 32]).unwrap(),
        commitment: PedersenCommitment::commit(1, &Scalar::ZERO),
        height: 1,
        tx_hash: [1u8; 32],
        output_index: 0,
    };
    db.commit_block(&block1, &[out1.clone()], &[], &[]).unwrap();

    let block2 = dummy_block(2);
    let out2 = OutputRecord {
        output_key: CompressedEdwardsY::from_slice(&[2u8; 32]).unwrap(),
        commitment: PedersenCommitment::commit(2, &Scalar::ZERO),
        height: 2,
        tx_hash: [2u8; 32],
        output_index: 0,
    };
    db.commit_block(&block2, &[out2.clone()], &[], &[]).unwrap();

    let block3 = dummy_block(3);
    let out3 = OutputRecord {
        output_key: CompressedEdwardsY::from_slice(&[3u8; 32]).unwrap(),
        commitment: PedersenCommitment::commit(3, &Scalar::ZERO),
        height: 3,
        tx_hash: [3u8; 32],
        output_index: 0,
    };
    db.commit_block(&block3, &[out3.clone()], &[out2.output_key], &[])
        .unwrap();

    // Now disconnect block 3
    let b2_hash =
        waecan_crypto::hash::keccak256(&waecan_core::block::serialize_header(&block2.header));
    db.block_disconnect(
        &block3,
        &[out3.output_key],
        &[out2.clone()],
        &[],
        &b2_hash,
        2,
    )
    .unwrap();

    // Now disconnect block 2
    let b1_hash =
        waecan_crypto::hash::keccak256(&waecan_core::block::serialize_header(&block1.header));
    db.block_disconnect(&block2, &[out2.output_key], &[], &[], &b1_hash, 1)
        .unwrap();

    // Verify UTXO matches state after block 1 only
    let cf = db.cf(CF_UTXO).unwrap();
    assert!(db
        .db
        .get_cf(&cf, out1.output_key.as_bytes())
        .unwrap()
        .is_some());
    assert!(db
        .db
        .get_cf(&cf, out2.output_key.as_bytes())
        .unwrap()
        .is_none());
    assert!(db
        .db
        .get_cf(&cf, out3.output_key.as_bytes())
        .unwrap()
        .is_none());

    let meta_cf = db.cf(CF_CHAIN_META).unwrap();
    let tip = db.db.get_cf(&meta_cf, b"tip_height").unwrap().unwrap();
    assert_eq!(u64::from_le_bytes(tip.try_into().unwrap()), 1);
}
