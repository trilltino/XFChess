import React from 'react';
import { Box, Spinner, Text, HStack, VStack } from '@chakra-ui/react';
import { useSolGbpPrice } from '../hooks/useSolGbpPrice';

const WAGER_ENTRY_GBP = 0.10;
const TOURNAMENT_ENTRY_GBP = 0.50;

function toSol(gbp: number, solGbp: number): string {
    return (gbp / solGbp).toFixed(4);
}

interface RowProps {
    label: string;
    gbp: number;
    solGbp: number | null;
    loading: boolean;
}

const PriceRow: React.FC<RowProps> = ({ label, gbp, solGbp, loading }) => (
    <HStack justifyContent="space-between" width="100%">
        <Text fontSize="sm" fontWeight={600} color="var(--text)" minW="120px">{label}</Text>
        <HStack gap={2}>
            <Text fontSize="sm" color="var(--primary)" fontWeight={700}>
                {gbp === WAGER_ENTRY_GBP ? '10p' : '50p'}
            </Text>
            <Text fontSize="xs" color="gray.400">≈</Text>
            {loading ? (
                <Spinner size="xs" color="var(--primary)" />
            ) : solGbp ? (
                <Text fontSize="sm" color="gray.300" fontFamily="monospace">
                    {toSol(gbp, solGbp)} SOL
                </Text>
            ) : (
                <Text fontSize="sm" color="gray.500">—</Text>
            )}
        </HStack>
    </HStack>
);

const WagerPriceWidget: React.FC = () => {
    const { solGbp, loading, updatedAt, error } = useSolGbpPrice();

    const updatedLabel = updatedAt
        ? `Updated ${updatedAt.toLocaleTimeString()}`
        : error ? 'Price unavailable' : 'Loading…';

    return (
        <Box
            p={4}
            borderWidth="1px"
            borderRadius="md"
            borderColor="var(--border)"
            bg="rgba(8,26,20,0.6)"
            backdropFilter="blur(8px)"
            mb={4}
        >
            <VStack gap={3} align="stretch">
                <PriceRow label="Wager Game" gbp={WAGER_ENTRY_GBP} solGbp={solGbp} loading={loading} />
                <PriceRow label="Tournament" gbp={TOURNAMENT_ENTRY_GBP} solGbp={solGbp} loading={loading} />
                <Text fontSize="xs" color="gray.500" cursor="default" textAlign="right" title={updatedLabel}>
                    ↻ live price
                </Text>
            </VStack>
        </Box>
    );
};

export default WagerPriceWidget;
