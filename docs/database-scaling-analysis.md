# Database Scaling Analysis for Encrypted PII Storage

## Overview
This document analyzes the database scaling requirements for storing encrypted Personally Identifiable Information (PII) for XFChess players, considering regulatory compliance (CACF) and performance requirements.

## Data Model

### Per-Player Encrypted PII Data
Each player record contains the following encrypted fields:

| Field | Type | Encrypted Size (avg) | Notes |
|-------|------|---------------------|-------|
| full_name | String | ~50 bytes | AES-256-GCM + nonce |
| dob | String | ~30 bytes | Date of birth |
| address | String | ~100 bytes | Full address |
| tax_id | String | ~50 bytes | Country-specific tax ID |
| country_code | String | 2 bytes (plaintext) | ISO 3166-1 alpha-2 |
| wallet_pubkey | String | 44 bytes (plaintext) | Solana wallet address |
| created_at | Timestamp | 8 bytes (plaintext) | Unix timestamp |
| updated_at | Timestamp | 8 bytes (plaintext) | Unix timestamp |

**Total per record**: ~292 bytes (encrypted) + 54 bytes (plaintext) = ~346 bytes

### Blind Indexes (Searchable Hashes)
For compliance reporting, we need searchable indexes:

| Index Field | Size | Purpose |
|-------------|------|---------|
| tax_id_blind_index | 32 bytes | SHA-256 hash for tax ID lookup |
| wallet_pubkey_index | 32 bytes | SHA-256 hash for wallet lookup |

**Total per record**: ~410 bytes including indexes

## Storage Requirements

### Growth Projections

| User Base | Records | Storage (MB) | Storage (GB) |
|-----------|---------|--------------|--------------|
| 1,000 | 1,000 | 0.4 MB | 0.0004 GB |
| 10,000 | 10,000 | 4 MB | 0.004 GB |
| 100,000 | 100,000 | 40 MB | 0.04 GB |
| 1,000,000 | 1,000,000 | 400 MB | 0.4 GB |
| 10,000,000 | 10,000,000 | 4 GB | 4 GB |

### Transaction Log Storage
For CACF compliance, we need to track all transactions:

| Transactions | Log Size (avg) | Storage (GB) |
|-------------|----------------|--------------|
| 100,000 | 500 bytes | 50 MB |
| 1,000,000 | 500 bytes | 500 MB |
| 10,000,000 | 500 bytes | 5 GB |

## Database Technology Recommendations

### Primary Database: PostgreSQL
**Rationale**: Mature, reliable, supports encryption extensions, excellent for compliance

**Configuration**:
- Connection pooling: PgBouncer for high concurrency
- Encryption: pgcrypto or application-level AES-256-GCM
- Indexes: B-tree on wallet_pubkey, GIN on blind indexes
- Partitioning: Range partitioning by created_at (monthly partitions)

### Secondary Database: Redis (Cache)
**Rationale**: Fast access for session keys, rate limiting

**Use Cases**:
- Session key cache (TTL: 24 hours)
- Rate limiting per IP/wallet
- Hot player profile caching

## Scaling Strategy

### Vertical Scaling (1M - 10M users)
- **Hardware**: 32 vCPU, 128 GB RAM, 2TB NVMe SSD
- **PostgreSQL**: Max connections = 200, shared_buffers = 32GB
- **Read Replicas**: 2-3 replicas for read-heavy queries
- **Connection Pooling**: PgBouncer with transaction pooling

### Horizontal Scaling (10M+ users)
- **Sharding Strategy**: Hash-based sharding by wallet_pubkey
- **Shard Count**: 16-32 shards initially
- **Consistent Hashing**: Minimize data movement during rebalancing
- **Global Router**: Application-level routing to correct shard

### Caching Layer
- **Redis Cluster**: 3-6 nodes for high availability
- **Cache Hit Rate Target**: 80%+
- **TTL Policies**: 
  - Session keys: 24 hours
  - Profile data: 1 hour
  - Transaction logs: 5 minutes

## Performance Targets

### Latency SLAs
| Operation | Target Latency |
|-----------|----------------|
| Encrypt & store PII | < 100ms |
| Decrypt PII | < 50ms |
| Search by tax ID | < 200ms |
| Search by wallet | < 100ms |
| Compliance report generation | < 5s |

### Throughput Targets
| Operation | Target TPS |
|-----------|-------------|
| PII write (encrypt) | 1,000 TPS |
| PII read (decrypt) | 5,000 TPS |
| Compliance query | 500 TPS |

## Backup & Recovery

### Backup Strategy
- **Frequency**: Every 6 hours
- **Retention**: 90 days daily, 1 year weekly
- **Storage**: Encrypted backups in cold storage (S3 Glacier)
- **Encryption**: Separate encryption key for backups

### Recovery Time Objectives (RTO)
- **Critical data**: < 1 hour
- **Non-critical data**: < 4 hours

### Recovery Point Objectives (RPO)
- **Critical data**: < 15 minutes
- **Non-critical data**: < 1 hour

## Security Considerations

### Encryption at Rest
- **Algorithm**: AES-256-GCM
- **Key Management**: AWS KMS or HashiCorp Vault
- **Key Rotation**: Every 90 days
- **Key Separation**: Different keys per shard/region

### Encryption in Transit
- **Protocol**: TLS 1.3
- **Cipher Suites**: Modern, forward-secure
- **Certificate Management**: Automated with Let's Encrypt or ACME

### Access Control
- **Principle of Least Privilege**: Role-based access control
- **Audit Logging**: All access logged with timestamp and user
- **IP Whitelisting**: Database access from specific subnets only

## Compliance Reporting

### UK CACF (HMRC)
- **Annual Reporting**: All transactions >£10,000
- **Data Retention**: 7 years
- **Required Fields**: Full name, NI, address, transaction amount

### Brazil CACF
- **Monthly Reporting**: All transactions >R$30,000
- **Data Retention**: 5 years
- **Required Fields**: Full name, CPF, address, transaction amount

### Germany CACF
- **Annual Reporting**: All transactions >€999.99
- **Data Retention**: 10 years
- **Required Fields**: Full name, tax ID, residency proof

### Canada CACF
- **Annual Reporting**: LVCTR for transactions >$10,000
- **Data Retention**: 6 years
- **Required Fields**: Full name, SIN, transaction amount

## Cost Estimates

### Infrastructure (Monthly)
| Component | 1M Users | 10M Users |
|-----------|----------|-----------|
| PostgreSQL Primary | $500 | $2,000 |
| PostgreSQL Replicas (2) | $1,000 | $4,000 |
| Redis Cluster | $200 | $800 |
| Backup Storage | $100 | $500 |
| **Total** | **$1,800** | **$7,300** |

### Storage Costs (Monthly)
| User Base | Storage | Cost (S3) |
|-----------|---------|-----------|
| 1M | 0.5 GB | $0.02 |
| 10M | 5 GB | $0.15 |
| 100M | 50 GB | $1.15 |

## Monitoring & Alerting

### Key Metrics
- Database connection pool utilization
- Query latency (p50, p95, p99)
- Encryption/decryption operation throughput
- Cache hit rate
- Storage growth rate

### Alert Thresholds
- Connection pool > 80%: Warning
- Query latency p95 > 500ms: Critical
- Cache hit rate < 70%: Warning
- Storage > 80% capacity: Critical

## Implementation Phases

### Phase 1: MVP (10K users)
- Single PostgreSQL instance
- Basic encryption/decryption
- Simple blind indexes
- No sharding

### Phase 2: Growth (100K users)
- Add read replicas
- Implement connection pooling
- Add Redis caching
- Partitioning by date

### Phase 3: Scale (1M users)
- Implement sharding
- Add write replicas
- Optimize indexes
- Implement advanced caching

### Phase 4: Enterprise (10M+ users)
- Multi-region deployment
- Global load balancing
- Advanced security features
- Automated scaling

## Conclusion

The proposed database architecture can scale from 10K to 10M+ users with appropriate infrastructure investment. The key considerations are:

1. **Encryption overhead** adds ~30% to storage size
2. **Blind indexes** enable searchable encryption without compromising privacy
3. **Sharding** is required beyond 1M users for performance
4. **Caching** is essential for reducing database load
5. **Compliance reporting** drives data retention requirements

The estimated monthly infrastructure cost is $1,800 for 1M users and $7,300 for 10M users, which is reasonable for a crypto gaming platform with monetization through fees and subscriptions.
