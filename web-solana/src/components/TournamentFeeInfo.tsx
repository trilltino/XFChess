import React from 'react';
import { Text, Box } from '@chakra-ui/react';

const TournamentFeeInfo: React.FC = () => {
  return (
    <Box p={4} borderWidth="1px" borderRadius="md" borderColor="gray.200" bg="gray.50" mb={4}>
      <Text fontSize="sm" color="gray.600">
        A platform fee of £0.50 is included in the registration cost. This fee helps cover transaction and rent costs for the tournament. Any unused portion contributes to platform revenue. Rent refunds may be provided to players upon account closure.
      </Text>
    </Box>
  );
};

export default TournamentFeeInfo;
